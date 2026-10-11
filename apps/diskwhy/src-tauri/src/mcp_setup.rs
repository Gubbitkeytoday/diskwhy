//! Connecting DiskWhy's MCP server (`diskwhy-cli.exe mcp`) to the Claude apps the user already has.
//!
//! The file is split in two halves:
//! * pure helpers (string/JSON building, path selection with injected directories) that are unit-tested;
//! * thin orchestration (`collect_info`, `register`, `unregister`) that touches the real profile. These are only
//!   ever reached from an explicit button click in the UI (`mcp_register` / `mcp_unregister`), never at startup.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Name under which DiskWhy is registered in both Claude apps.
pub const SERVER_NAME: &str = "diskwhy";
const CONFIG_FILE: &str = "claude_desktop_config.json";
const CLI_FILE: &str = "diskwhy-cli.exe";
/// Shown inside the example commands when the CLI cannot be located (dev build).
const CLI_PLACEHOLDER: &str = "diskwhy-cli.exe";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Which Claude app to (un)register DiskWhy with. Serialized as `"claude-code"` / `"claude-desktop"`.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum McpTarget {
    ClaudeCode,
    ClaudeDesktop,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpInfo {
    pub cli_path: Option<String>,
    pub cli_found: bool,
    pub claude_code_path: Option<String>,
    pub claude_code_registered: bool,
    pub claude_desktop_config_path: Option<String>,
    pub claude_desktop_registered: bool,
    pub claude_code_command: String,
    pub desktop_snippet: String,
    /// One-off Thai message after register/unregister (for example "restart Claude Desktop"). `None` from `mcp_info`.
    pub note: Option<String>,
}

/* ───────────────────────────── pure helpers ───────────────────────────── */

/// Strips the `\\?\` verbatim prefix some Windows APIs return; Claude's config and CLI users expect plain paths.
pub fn clean_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p.to_path_buf()
    }
}

/// `claude mcp add --scope user diskwhy -- "<cli>" mcp` — the command the user can paste into a terminal.
pub fn claude_code_command(cli: &str) -> String {
    format!("claude mcp add --scope user {SERVER_NAME} -- \"{cli}\" mcp")
}

/// The `mcpServers.diskwhy` entry for `claude_desktop_config.json`.
pub fn desktop_server_entry(cli: &str) -> Value {
    json!({ "command": cli, "args": ["mcp"] })
}

/// Pretty JSON `{ "mcpServers": { "diskwhy": { "command": "...", "args": ["mcp"] } } }`; serde_json escapes backslashes.
pub fn desktop_snippet(cli: &str) -> String {
    let doc = json!({ "mcpServers": { SERVER_NAME: desktop_server_entry(cli) } });
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

/// Parses a Claude Desktop config. A missing/blank file is an empty object; anything that is not a JSON object is an error
/// (we refuse to overwrite a file we do not understand).
pub fn parse_config(text: &str) -> Result<Value, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    match serde_json::from_str::<Value>(text) {
        Ok(v @ Value::Object(_)) => Ok(v),
        Ok(_) => Err("ไฟล์ตั้งค่า Claude Desktop ไม่ใช่ออบเจ็กต์ JSON — DiskWhy จึงไม่แก้ไขไฟล์นี้".into()),
        Err(e) => Err(format!("ไฟล์ตั้งค่า Claude Desktop อ่านเป็น JSON ไม่ได้ ({e}) — DiskWhy จึงไม่แก้ไขไฟล์นี้ ลองแก้ไฟล์ให้ถูกต้องก่อน")),
    }
}

pub fn is_registered(cfg: &Value) -> bool {
    cfg.get("mcpServers").and_then(|s| s.get(SERVER_NAME)).is_some()
}

/// Adds/replaces `mcpServers.diskwhy` without touching any other key or server.
pub fn merge_server(cfg: &mut Value, cli: &str) -> Result<(), String> {
    let root = cfg.as_object_mut().ok_or("ไฟล์ตั้งค่าไม่ใช่ออบเจ็กต์ JSON")?;
    let servers = root.entry("mcpServers").or_insert_with(|| Value::Object(Map::new()));
    let servers = servers.as_object_mut().ok_or("ค่า mcpServers ในไฟล์ตั้งค่าไม่ใช่ออบเจ็กต์ — DiskWhy จึงไม่แก้ไข")?;
    servers.insert(SERVER_NAME.into(), desktop_server_entry(cli));
    Ok(())
}

/// Removes only `mcpServers.diskwhy`. Returns whether it was present.
pub fn remove_server(cfg: &mut Value) -> Result<bool, String> {
    let root = cfg.as_object_mut().ok_or("ไฟล์ตั้งค่าไม่ใช่ออบเจ็กต์ JSON")?;
    match root.get_mut("mcpServers") {
        None => Ok(false),
        Some(Value::Object(servers)) => Ok(servers.shift_remove(SERVER_NAME).is_some()),
        Some(_) => Err("ค่า mcpServers ในไฟล์ตั้งค่าไม่ใช่ออบเจ็กต์ — DiskWhy จึงไม่แก้ไข".into()),
    }
}

/// True when `~/.claude.json` has a user-scope `mcpServers.diskwhy`. Fallback when `claude mcp get` cannot run in time.
pub fn claude_json_has_user_server(text: &str) -> bool {
    parse_config(text).map(|v| is_registered(&v)).unwrap_or(false)
}

/// Candidate `claude_desktop_config.json` locations, best first: MSIX-packaged installs
/// (`%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude`), then `%APPDATA%\Claude`.
pub fn desktop_config_candidates(appdata: Option<&Path>, localappdata: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = localappdata
        && let Ok(rd) = std::fs::read_dir(local.join("Packages"))
    {
        let mut names: Vec<String> = rd
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.starts_with("Claude_"))
            .collect();
        names.sort();
        for n in names {
            out.push(local.join("Packages").join(n).join("LocalCache").join("Roaming").join("Claude").join(CONFIG_FILE));
        }
    }
    if let Some(app) = appdata {
        out.push(app.join("Claude").join(CONFIG_FILE));
    }
    out
}

/// Picks the config to use: the first candidate whose file exists (so a packaged one wins over the classic one),
/// otherwise the first whose folder exists (Claude Desktop is installed but never saved a config yet).
pub fn pick_desktop_config(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|p| p.is_file())
        .or_else(|| candidates.iter().find(|p| p.parent().is_some_and(Path::is_dir)))
        .cloned()
}

/// Folders searched for the Claude Code CLI: every `PATH` entry, then the native installer and npm locations.
pub fn claude_search_dirs(path_env: Option<&OsStr>, userprofile: Option<&Path>, appdata: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = path_env.map(|p| std::env::split_paths(p).filter(|d| !d.as_os_str().is_empty()).collect()).unwrap_or_default();
    if let Some(u) = userprofile {
        dirs.push(u.join(".local").join("bin"));
    }
    if let Some(a) = appdata {
        dirs.push(a.join("npm"));
    }
    dirs
}

pub fn find_claude(dirs: &[PathBuf]) -> Option<PathBuf> {
    dirs.iter().find_map(|d| ["claude.exe", "claude.cmd"].iter().map(|n| d.join(n)).find(|p| p.is_file()))
}

/// `claude_desktop_config.json.bak-<unix>` next to the config.
pub fn backup_path(config: &Path, unix: u64) -> PathBuf {
    let name = config.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| CONFIG_FILE.into());
    config.with_file_name(format!("{name}.bak-{unix}"))
}

/// `claude mcp add` exits non-zero when the server is already registered; that is the state we wanted.
pub fn is_already_exists(output: &str) -> bool {
    output.to_lowercase().contains("already exists")
}

/// `claude mcp remove` when nothing was registered: also the state we wanted.
pub fn is_not_found(output: &str) -> bool {
    let o = output.to_lowercase();
    o.contains("no mcp server found") || o.contains("not found")
}

/// A CLI that is missing or an empty placeholder (dev builds create a 0-byte file so the bundler config resolves) is unusable.
pub fn is_usable_cli(path: &Path) -> bool {
    std::fs::metadata(path).map(|m| m.is_file() && m.len() > 0).unwrap_or(false)
}

/// First usable CLI among: the bundled resource, then the folder of the running exe.
pub fn locate_cli(resource_dir: Option<&Path>, exe_dir: Option<&Path>) -> Option<PathBuf> {
    let candidates = [
        resource_dir.map(|r| r.join("binaries").join(CLI_FILE)),
        exe_dir.map(|e| e.join(CLI_FILE)),
    ];
    candidates.into_iter().flatten().find(|p| is_usable_cli(p)).map(|p| clean_path(&p))
}

/// Writes `contents` to a temp file beside `path`, then renames it over `path` (atomic on the same volume).
pub fn write_atomic(path: &Path, contents: &str, unix: u64) -> std::io::Result<()> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!("{name}.tmp-{unix}"));
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn pretty(cfg: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(cfg).map(|s| s + "\n").map_err(|e| e.to_string())
}

fn read_config(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("อ่านไฟล์ตั้งค่าไม่ได้ ({}): {e}", path.display()))?;
    parse_config(&String::from_utf8_lossy(&bytes))
}

/// Merges `mcpServers.diskwhy` into the config at `path` (created as `{}` when missing). Backs the old file up first.
/// Returns the backup path, if a backup was made.
pub fn register_desktop(path: &Path, cli: &str, unix: u64) -> Result<Option<PathBuf>, String> {
    let mut cfg = read_config(path)?;
    merge_server(&mut cfg, cli)?;
    let backup = backup_existing(path, unix)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("สร้างโฟลเดอร์ไม่ได้ ({}): {e}", dir.display()))?;
    }
    write_atomic(path, &pretty(&cfg)?, unix).map_err(|e| format!("เขียนไฟล์ตั้งค่าไม่สำเร็จ: {e}"))?;
    Ok(backup)
}

/// Removes `mcpServers.diskwhy` from the config at `path`, backing it up first. A no-op (no write, no backup) if absent.
pub fn unregister_desktop(path: &Path, unix: u64) -> Result<Option<PathBuf>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let mut cfg = read_config(path)?;
    if !remove_server(&mut cfg)? {
        return Ok(None);
    }
    let backup = backup_existing(path, unix)?;
    write_atomic(path, &pretty(&cfg)?, unix).map_err(|e| format!("เขียนไฟล์ตั้งค่าไม่สำเร็จ: {e}"))?;
    Ok(backup)
}

fn backup_existing(path: &Path, unix: u64) -> Result<Option<PathBuf>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let bak = backup_path(path, unix);
    std::fs::copy(path, &bak).map_err(|e| format!("สำรองไฟล์เดิมไม่สำเร็จ ({}): {e}", bak.display()))?;
    Ok(Some(bak))
}

/* ───────────────────────── process / profile access ───────────────────────── */

/// Directories the lookups depend on. Built from the process environment in production, injected in tests.
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub path: Option<OsString>,
    pub userprofile: Option<PathBuf>,
    pub appdata: Option<PathBuf>,
    pub localappdata: Option<PathBuf>,
}

impl Env {
    pub fn from_process() -> Self {
        let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty());
        Self {
            path: var("PATH"),
            userprofile: var("USERPROFILE").map(PathBuf::from),
            appdata: var("APPDATA").map(PathBuf::from),
            localappdata: var("LOCALAPPDATA").map(PathBuf::from),
        }
    }
    fn claude(&self) -> Option<PathBuf> {
        find_claude(&claude_search_dirs(self.path.as_deref(), self.userprofile.as_deref(), self.appdata.as_deref()))
    }
    fn desktop_config(&self) -> Option<PathBuf> {
        pick_desktop_config(&desktop_config_candidates(self.appdata.as_deref(), self.localappdata.as_deref()))
    }
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(windows)]
fn hide_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
fn hide_window(_: &mut Command) {}

/// Runs `program args…` without a console window, capturing output; kills it after `timeout`.
fn run_hidden(program: &Path, args: &[&str], timeout: Duration) -> Result<Output, String> {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    hide_window(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("เรียก {} ไม่สำเร็จ: {e}", program.display()))?;
    let drain = |s: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut s) = s {
                let _ = s.read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = drain(child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let err = drain(child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>));
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => {
                return Ok(Output { status, stdout: out.join().unwrap_or_default(), stderr: err.join().unwrap_or_default() });
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{} ใช้เวลานานเกิน {} วินาที จึงยกเลิก", program.display(), timeout.as_secs()));
            }
            None => std::thread::sleep(Duration::from_millis(40)),
        }
    }
}

fn output_text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).trim().to_string()
}

fn claude_code_registered(claude: &Path, env: &Env) -> bool {
    match run_hidden(claude, &["mcp", "get", SERVER_NAME], COMMAND_TIMEOUT) {
        Ok(o) => o.status.success(),
        // `claude` could not answer in time: fall back to the user-scope entry in ~/.claude.json.
        Err(_) => env
            .userprofile
            .as_ref()
            .and_then(|u| std::fs::read_to_string(u.join(".claude.json")).ok())
            .is_some_and(|t| claude_json_has_user_server(&t)),
    }
}

/// Snapshot of what is installed and registered. Read-only.
pub fn collect_info(cli: Option<&Path>, env: &Env) -> McpInfo {
    let cli_str = cli.map(|p| p.to_string_lossy().into_owned());
    let cli_for_text = cli_str.as_deref().unwrap_or(CLI_PLACEHOLDER);
    let claude = env.claude();
    let desktop = env.desktop_config();
    let desktop_registered = desktop
        .as_ref()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| parse_config(&String::from_utf8_lossy(&b)).ok())
        .is_some_and(|v| is_registered(&v));
    McpInfo {
        cli_found: cli_str.is_some(),
        cli_path: cli_str.clone(),
        claude_code_registered: claude.as_deref().is_some_and(|c| claude_code_registered(c, env)),
        claude_code_path: claude.map(|p| p.to_string_lossy().into_owned()),
        claude_desktop_config_path: desktop.map(|p| p.to_string_lossy().into_owned()),
        claude_desktop_registered: desktop_registered,
        claude_code_command: claude_code_command(cli_for_text),
        desktop_snippet: desktop_snippet(cli_for_text),
        note: None,
    }
}

fn need_cli(cli: Option<&Path>) -> Result<&Path, String> {
    cli.ok_or_else(|| "ไม่พบ diskwhy-cli.exe ในเครื่องนี้ (เวอร์ชันนักพัฒนา) — รัน `pnpm cli:build` ก่อน แล้วเปิดแอปใหม่".to_string())
}

/// Registers DiskWhy with the chosen Claude app. Returns a Thai note for the UI. Called only from a button click.
pub fn register(target: McpTarget, cli: Option<&Path>, env: &Env) -> Result<String, String> {
    let cli = need_cli(cli)?;
    match target {
        McpTarget::ClaudeCode => {
            let claude = env.claude().ok_or("ไม่พบ Claude Code (claude.exe) ในเครื่องนี้ — ติดตั้งก่อน หรือคัดลอกคำสั่งไปรันเอง")?;
            let cli_s = cli.to_string_lossy();
            let out = run_hidden(&claude, &["mcp", "add", "--scope", "user", SERVER_NAME, "--", &cli_s, "mcp"], COMMAND_TIMEOUT)?;
            let text = output_text(&out);
            if out.status.success() || is_already_exists(&text) {
                Ok("เชื่อมต่อ Claude Code แล้ว — เปิดเซสชันใหม่ใน Claude Code แล้วพิมพ์ /mcp เพื่อตรวจสอบ".into())
            } else {
                Err(format!("claude mcp add ไม่สำเร็จ: {text}"))
            }
        }
        McpTarget::ClaudeDesktop => {
            let path = env.desktop_config().ok_or("ไม่พบโฟลเดอร์ตั้งค่าของ Claude Desktop — ติดตั้งและเปิด Claude Desktop อย่างน้อยหนึ่งครั้งก่อน")?;
            let backup = register_desktop(&path, &cli.to_string_lossy(), unix_now())?;
            Ok(format!(
                "เชื่อมต่อ Claude Desktop แล้ว — ต้องปิด Claude Desktop ให้สนิท (คลิกขวาไอคอนที่ถาดระบบ แล้วเลือกออก) และเปิดใหม่ จึงจะเห็น DiskWhy{}",
                backup.map(|b| format!(" · สำรองไฟล์เดิมไว้ที่ {}", b.display())).unwrap_or_default()
            ))
        }
    }
}

/// Removes the registration. Called only from a button click.
pub fn unregister(target: McpTarget, env: &Env) -> Result<String, String> {
    match target {
        McpTarget::ClaudeCode => {
            let claude = env.claude().ok_or("ไม่พบ Claude Code (claude.exe) ในเครื่องนี้")?;
            let out = run_hidden(&claude, &["mcp", "remove", "--scope", "user", SERVER_NAME], COMMAND_TIMEOUT)?;
            let text = output_text(&out);
            if out.status.success() || is_not_found(&text) {
                Ok("ยกเลิกการเชื่อมต่อ Claude Code แล้ว".into())
            } else {
                Err(format!("claude mcp remove ไม่สำเร็จ: {text}"))
            }
        }
        McpTarget::ClaudeDesktop => {
            let path = env.desktop_config().ok_or("ไม่พบไฟล์ตั้งค่าของ Claude Desktop")?;
            let backup = unregister_desktop(&path, unix_now())?;
            Ok(format!(
                "ยกเลิกการเชื่อมต่อ Claude Desktop แล้ว — ปิดและเปิด Claude Desktop ใหม่เพื่อให้มีผล{}",
                backup.map(|b| format!(" · สำรองไฟล์เดิมไว้ที่ {}", b.display())).unwrap_or_default()
            ))
        }
    }
}

/* ───────────────────────────────── tests ───────────────────────────────── */

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Unique scratch directory under the system temp dir, removed on drop. Never touches the real profile.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!("diskwhy-mcp-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn file(&self, rel: &str, contents: &str) -> PathBuf {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, contents).unwrap();
            p
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const CLI: &str = r"C:\Users\Som Chai\AppData\Local\DiskWhy\binaries\diskwhy-cli.exe";

    #[test]
    fn snippet_escapes_backslashes_and_round_trips() {
        let s = desktop_snippet(CLI);
        assert!(s.contains(r"C:\\Users\\Som Chai\\AppData\\Local\\DiskWhy\\binaries\\diskwhy-cli.exe"), "{s}");
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["mcpServers"]["diskwhy"]["command"], CLI);
        assert_eq!(v["mcpServers"]["diskwhy"]["args"], json!(["mcp"]));
        assert!(s.contains("\n  \"mcpServers\""), "pretty-printed with 2 spaces: {s}");
    }

    #[test]
    fn claude_code_command_quotes_the_path() {
        assert_eq!(
            claude_code_command(CLI),
            format!("claude mcp add --scope user diskwhy -- \"{CLI}\" mcp")
        );
    }

    #[test]
    fn merge_preserves_other_servers_and_keys_in_order() {
        let mut cfg = parse_config(
            r#"{ "theme": "dark", "mcpServers": { "other": { "command": "x", "args": ["a"], "env": {"K":"V"} }, "zeta": {"command":"z"} }, "globalShortcut": "Ctrl+Space" }"#,
        )
        .unwrap();
        merge_server(&mut cfg, CLI).unwrap();
        assert!(is_registered(&cfg));
        assert_eq!(cfg["theme"], "dark");
        assert_eq!(cfg["globalShortcut"], "Ctrl+Space");
        assert_eq!(cfg["mcpServers"]["other"]["env"]["K"], "V");
        assert_eq!(cfg["mcpServers"]["zeta"]["command"], "z");
        assert_eq!(cfg["mcpServers"]["diskwhy"]["command"], CLI);
        let keys: Vec<_> = cfg.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["theme", "mcpServers", "globalShortcut"], "key order must be preserved");
        let servers: Vec<_> = cfg["mcpServers"].as_object().unwrap().keys().cloned().collect();
        assert_eq!(servers, ["other", "zeta", "diskwhy"]);
    }

    #[test]
    fn merge_creates_mcp_servers_and_replaces_stale_entry() {
        let mut cfg = parse_config("{}").unwrap();
        merge_server(&mut cfg, r"C:\old\diskwhy-cli.exe").unwrap();
        merge_server(&mut cfg, CLI).unwrap();
        assert_eq!(cfg["mcpServers"]["diskwhy"]["command"], CLI);
        assert_eq!(cfg["mcpServers"].as_object().unwrap().len(), 1);
    }

    #[test]
    fn merge_and_remove_refuse_malformed_shapes() {
        let mut cfg = json!({ "mcpServers": [] });
        assert!(merge_server(&mut cfg, CLI).is_err());
        assert!(remove_server(&mut cfg).is_err());
        let mut arr = json!([]);
        assert!(merge_server(&mut arr, CLI).is_err());
    }

    #[test]
    fn remove_only_drops_diskwhy() {
        let mut cfg = json!({ "a": 1, "mcpServers": { "diskwhy": {"command":"x"}, "other": {"command":"y"} } });
        assert!(remove_server(&mut cfg).unwrap());
        assert!(!is_registered(&cfg));
        assert_eq!(cfg["a"], 1);
        assert_eq!(cfg["mcpServers"]["other"]["command"], "y");
        assert!(!remove_server(&mut cfg).unwrap(), "second removal is a no-op");
        let mut none = json!({ "a": 1 });
        assert!(!remove_server(&mut none).unwrap());
    }

    #[test]
    fn parse_config_handles_bom_blank_and_garbage() {
        assert_eq!(parse_config("").unwrap(), json!({}));
        assert_eq!(parse_config("  \n").unwrap(), json!({}));
        assert_eq!(parse_config("\u{feff}{\"a\":1}").unwrap(), json!({"a":1}));
        assert!(parse_config("{ nope").is_err());
        assert!(parse_config("[1,2]").is_err());
    }

    #[test]
    fn claude_json_fallback_reads_user_scope_only() {
        assert!(claude_json_has_user_server(r#"{"mcpServers":{"diskwhy":{}}}"#));
        assert!(!claude_json_has_user_server(r#"{"projects":{"C:/x":{"mcpServers":{"diskwhy":{}}}}}"#));
        assert!(!claude_json_has_user_server("not json"));
    }

    #[test]
    fn config_prefers_packaged_location_when_it_exists() {
        let s = Scratch::new();
        let local = s.0.join("Local");
        let roaming = s.0.join("Roaming");
        let packaged = s.file(r"Local\Packages\Claude_abc123\LocalCache\Roaming\Claude\claude_desktop_config.json", "{}");
        s.file(r"Roaming\Claude\claude_desktop_config.json", "{}");
        s.file(r"Local\Packages\Other_xyz\LocalCache\Roaming\Claude\claude_desktop_config.json", "{}");

        let cands = desktop_config_candidates(Some(&roaming), Some(&local));
        assert_eq!(cands.len(), 2, "one packaged + one classic; non-Claude_ packages ignored: {cands:?}");
        assert_eq!(pick_desktop_config(&cands), Some(packaged));
    }

    #[test]
    fn config_falls_back_to_classic_then_to_existing_folder_then_none() {
        let s = Scratch::new();
        let local = s.0.join("Local");
        let roaming = s.0.join("Roaming");
        // Packaged folder exists but Claude never created its Roaming\Claude folder, classic file exists.
        std::fs::create_dir_all(local.join(r"Packages\Claude_abc123")).unwrap();
        let classic = s.file(r"Roaming\Claude\claude_desktop_config.json", "{}");
        let cands = desktop_config_candidates(Some(&roaming), Some(&local));
        assert_eq!(pick_desktop_config(&cands), Some(classic.clone()));

        // No file anywhere, but the classic folder exists -> use it (config gets created there).
        std::fs::remove_file(&classic).unwrap();
        assert_eq!(pick_desktop_config(&cands), Some(classic));

        // Nothing installed.
        let empty = Scratch::new();
        let cands = desktop_config_candidates(Some(&empty.0.join("R")), Some(&empty.0.join("L")));
        assert_eq!(pick_desktop_config(&cands), None);
        assert!(desktop_config_candidates(None, None).is_empty());
    }

    #[test]
    fn find_claude_searches_path_then_known_dirs() {
        let s = Scratch::new();
        let profile = s.0.join("profile");
        let appdata = s.0.join("appdata");
        let on_path = s.0.join("bin");
        std::fs::create_dir_all(&on_path).unwrap();

        let path_env = std::env::join_paths([&on_path]).unwrap();
        let dirs = claude_search_dirs(Some(&path_env), Some(&profile), Some(&appdata));
        assert_eq!(dirs, [on_path.clone(), profile.join(".local").join("bin"), appdata.join("npm")]);
        assert_eq!(find_claude(&dirs), None);

        let npm = s.file(r"appdata\npm\claude.cmd", "");
        assert_eq!(find_claude(&dirs), Some(npm));
        let native = s.file(r"profile\.local\bin\claude.exe", "");
        assert_eq!(find_claude(&dirs), Some(native), ".local\\bin comes before npm");
        let first = s.file(r"bin\claude.exe", "");
        assert_eq!(find_claude(&dirs), Some(first), "PATH wins");
    }

    #[test]
    fn locate_cli_prefers_resource_and_ignores_empty_placeholders() {
        let s = Scratch::new();
        let res = s.0.join("res");
        let exe = s.0.join("exe");
        assert_eq!(locate_cli(Some(&res), Some(&exe)), None);

        s.file(r"res\binaries\diskwhy-cli.exe", ""); // dev placeholder
        assert_eq!(locate_cli(Some(&res), Some(&exe)), None, "0-byte placeholder is not a CLI");

        let beside = s.file(r"exe\diskwhy-cli.exe", "MZ");
        assert_eq!(locate_cli(Some(&res), Some(&exe)), Some(beside));

        let bundled = s.file(r"res\binaries\diskwhy-cli.exe", "MZ");
        assert_eq!(locate_cli(Some(&res), Some(&exe)), Some(bundled));
    }

    #[test]
    fn clean_path_strips_verbatim_prefix() {
        assert_eq!(clean_path(Path::new(r"\\?\C:\a\b.exe")), PathBuf::from(r"C:\a\b.exe"));
        assert_eq!(clean_path(Path::new(r"\\?\UNC\srv\share\b.exe")), PathBuf::from(r"\\srv\share\b.exe"));
        assert_eq!(clean_path(Path::new(r"C:\a\b.exe")), PathBuf::from(r"C:\a\b.exe"));
    }

    #[test]
    fn already_exists_and_not_found_detection() {
        assert!(is_already_exists("MCP server diskwhy already exists in user config"));
        assert!(!is_already_exists("something else"));
        assert!(is_not_found("No MCP server found with name: diskwhy"));
    }

    #[test]
    fn register_desktop_creates_missing_config_without_backup() {
        let s = Scratch::new();
        let path = s.0.join(r"Claude\claude_desktop_config.json");
        let backup = register_desktop(&path, CLI, 100).unwrap();
        assert_eq!(backup, None);
        let cfg = parse_config(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(cfg["mcpServers"]["diskwhy"]["command"], CLI);
        assert!(!s.0.join(r"Claude\claude_desktop_config.json.tmp-100").exists(), "temp file renamed away");
    }

    #[test]
    fn register_desktop_backs_up_and_preserves_everything_else() {
        let s = Scratch::new();
        let original = "{\n  \"mcpServers\": {\n    \"fs\": { \"command\": \"npx\", \"args\": [\"-y\", \"srv\"] }\n  },\n  \"theme\": \"dark\"\n}\n";
        let path = s.file("claude_desktop_config.json", original);
        let backup = register_desktop(&path, CLI, 1_700_000_000).unwrap().expect("backup made");
        assert_eq!(backup, s.0.join("claude_desktop_config.json.bak-1700000000"));
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), original, "backup is the untouched original");
        let cfg = parse_config(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(cfg["theme"], "dark");
        assert_eq!(cfg["mcpServers"]["fs"]["args"], json!(["-y", "srv"]));
        assert_eq!(cfg["mcpServers"]["diskwhy"]["args"], json!(["mcp"]));
    }

    #[test]
    fn register_desktop_refuses_invalid_json_and_leaves_file_alone() {
        let s = Scratch::new();
        let path = s.file("claude_desktop_config.json", "{ broken");
        assert!(register_desktop(&path, CLI, 1).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ broken");
        assert!(!s.0.join("claude_desktop_config.json.bak-1").exists());
    }

    #[test]
    fn unregister_desktop_removes_only_diskwhy_with_backup_and_is_noop_when_absent() {
        let s = Scratch::new();
        let path = s.file(
            "claude_desktop_config.json",
            r#"{"mcpServers":{"diskwhy":{"command":"x","args":["mcp"]},"fs":{"command":"npx"}},"theme":"dark"}"#,
        );
        let backup = unregister_desktop(&path, 7).unwrap().expect("backup made");
        assert!(backup.is_file());
        let cfg = parse_config(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(!is_registered(&cfg));
        assert_eq!(cfg["mcpServers"]["fs"]["command"], "npx");
        assert_eq!(cfg["theme"], "dark");

        // Already gone: nothing written, no new backup.
        assert_eq!(unregister_desktop(&path, 8).unwrap(), None);
        assert!(!s.0.join("claude_desktop_config.json.bak-8").exists());
        // Missing file: nothing to do.
        assert_eq!(unregister_desktop(&s.0.join("nope.json"), 9).unwrap(), None);
    }

    #[test]
    fn target_deserializes_from_ui_strings() {
        assert_eq!(serde_json::from_str::<McpTarget>("\"claude-code\"").unwrap(), McpTarget::ClaudeCode);
        assert_eq!(serde_json::from_str::<McpTarget>("\"claude-desktop\"").unwrap(), McpTarget::ClaudeDesktop);
        assert!(serde_json::from_str::<McpTarget>("\"other\"").is_err());
    }

    #[test]
    fn info_serializes_camel_case_with_all_fields() {
        let info = collect_info(None, &Env::default());
        let v = serde_json::to_value(&info).unwrap();
        for k in [
            "cliPath",
            "cliFound",
            "claudeCodePath",
            "claudeCodeRegistered",
            "claudeDesktopConfigPath",
            "claudeDesktopRegistered",
            "claudeCodeCommand",
            "desktopSnippet",
            "note",
        ] {
            assert!(v.get(k).is_some(), "missing {k}");
        }
        assert_eq!(v["cliFound"], false);
        assert_eq!(v["cliPath"], Value::Null);
        assert_eq!(v["claudeCodeRegistered"], false);
        assert!(v["claudeCodeCommand"].as_str().unwrap().ends_with("-- \"diskwhy-cli.exe\" mcp"));
    }

    #[test]
    fn register_without_cli_explains_how_to_build_it() {
        let err = register(McpTarget::ClaudeDesktop, None, &Env::default()).unwrap_err();
        assert!(err.contains("pnpm cli:build"), "{err}");
    }
}
