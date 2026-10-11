//! Windows actions DiskWhy may perform. Every action is a fixed, whitelisted program + arguments:
//! no shell string, no interpolation of user/model input.
//!
//! Two layers:
//! * [`system_tool_command`] / [`open_system_tool`]: open a built-in Windows tool (used by the desktop app).
//! * [`CLEANUP_ACTIONS`] / [`run_cleanup`]: the whitelisted cleanup actions the MCP server and the CLI may run.

use crate::{report, safety};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

/// Built-in Windows tools DiskWhy can open (the user finishes the job in Windows' own UI).
pub const SYSTEM_TOOLS: &[&str] = &["storage-sense", "disk-cleanup", "apps-features", "recycle-bin"];

/// Strict whitelist: tool id -> (program, args). Anything else is rejected.
pub fn system_tool_command(tool: &str) -> Option<(&'static str, &'static [&'static str])> {
    match tool {
        "storage-sense" => Some(("explorer.exe", &["ms-settings:storagesense"])),
        "disk-cleanup" => Some(("cleanmgr.exe", &[])),
        "apps-features" => Some(("explorer.exe", &["ms-settings:appsfeatures"])),
        "recycle-bin" => Some(("explorer.exe", &["shell:RecycleBinFolder"])),
        _ => None,
    }
}

pub fn open_system_tool(tool: &str) -> Result<(), String> {
    let (program, args) = system_tool_command(tool.trim()).ok_or_else(|| "ไม่รองรับเครื่องมือนี้".to_string())?;
    Command::new(program).args(args).spawn().map(|_| ()).map_err(|e| format!("เปิดเครื่องมือของ Windows ไม่สำเร็จ: {e}"))
}

/* ── Whitelisted cleanup actions ─────────────────────────────────────────── */

/// Hard limit for one cleanup command.
pub const CLEANUP_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// `%TEMP%` entries must be untouched this long before they are moved to the Recycle Bin.
pub const TEMP_MIN_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// A fixed program + argument list. Never built from user input.
#[derive(Clone, Copy, Debug)]
pub struct CommandSpec {
    pub program: &'static str,
    pub args: &'static [&'static str],
}

#[derive(Clone, Copy, Debug)]
pub enum CleanupKind {
    /// Try each command in order; the next one is used only when the previous program is not installed.
    Command(&'static [CommandSpec]),
    /// Move `%TEMP%` entries older than [`TEMP_MIN_AGE`] to the Recycle Bin.
    TempFiles,
    /// Fixed PowerShell script: the ONLY permanent deletion DiskWhy performs.
    EmptyRecycleBin,
    /// Open a built-in Windows tool (an id from [`SYSTEM_TOOLS`]).
    OpenTool(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct CleanupSpec {
    /// Public id (the MCP `action` / CLI argument).
    pub id: &'static str,
    /// Thai title.
    pub title: &'static str,
    /// What it does (Thai) including the trade-off.
    pub description: &'static str,
    pub kind: CleanupKind,
    /// Removes data (as opposed to only opening a window).
    pub destructive: bool,
    /// Deletes permanently (cannot be undone).
    pub permanent: bool,
}

const fn cmd(program: &'static str, args: &'static [&'static str]) -> CommandSpec {
    CommandSpec { program, args }
}

pub const CLEANUP_ACTIONS: &[CleanupSpec] = &[
    CleanupSpec {
        id: "npm-cache",
        title: "ล้างแคช npm",
        description: "รัน `npm cache clean --force` — แคชแพ็กเกจ npm ดาวน์โหลดกลับมาได้เองเมื่อต้องใช้ (การติดตั้งครั้งถัดไปอาจช้าลงเล็กน้อย)",
        kind: CleanupKind::Command(&[cmd("cmd.exe", &["/C", "npm", "cache", "clean", "--force"])]),
        destructive: true,
        permanent: true,
    },
    CleanupSpec {
        id: "pnpm-store",
        title: "ล้าง pnpm store",
        description: "รัน `pnpm store prune` — ลบแพ็กเกจใน store ที่ไม่มีโปรเจกต์ไหนอ้างอิงแล้ว",
        kind: CleanupKind::Command(&[cmd("cmd.exe", &["/C", "pnpm", "store", "prune"])]),
        destructive: true,
        permanent: true,
    },
    CleanupSpec {
        id: "yarn-cache",
        title: "ล้างแคช Yarn",
        description: "รัน `yarn cache clean` — ลบแคชแพ็กเกจของ Yarn ดาวน์โหลดกลับมาได้เอง",
        kind: CleanupKind::Command(&[cmd("cmd.exe", &["/C", "yarn", "cache", "clean"])]),
        destructive: true,
        permanent: true,
    },
    CleanupSpec {
        id: "pip-cache",
        title: "ล้างแคช pip",
        description: "รัน `python -m pip cache purge` (ถ้าไม่มี python จะลอง `py -m pip cache purge`) — ลบแคชแพ็กเกจ Python ดาวน์โหลดกลับมาได้เอง",
        kind: CleanupKind::Command(&[
            cmd("python.exe", &["-m", "pip", "cache", "purge"]),
            cmd("py.exe", &["-m", "pip", "cache", "purge"]),
        ]),
        destructive: true,
        permanent: true,
    },
    CleanupSpec {
        id: "nuget-cache",
        title: "ล้างแคช NuGet",
        description: "รัน `dotnet nuget locals all --clear` — ลบแคชแพ็กเกจ NuGet (โปรเจกต์ .NET จะ restore ใหม่เมื่อ build ครั้งถัดไป)",
        kind: CleanupKind::Command(&[cmd("dotnet.exe", &["nuget", "locals", "all", "--clear"])]),
        destructive: true,
        permanent: true,
    },
    CleanupSpec {
        id: "wsl-shutdown",
        title: "ปิด WSL ทั้งหมด",
        description: "รัน `wsl --shutdown` — ไม่ได้คืนพื้นที่เอง แต่จำเป็นก่อนบีบไฟล์ .vhdx (Optimize-VHD) การทำงานที่เปิดอยู่ใน WSL/Docker Desktop จะหยุด",
        kind: CleanupKind::Command(&[cmd("wsl.exe", &["--shutdown"])]),
        destructive: false,
        permanent: false,
    },
    CleanupSpec {
        id: "temp-files",
        title: "ล้างไฟล์ชั่วคราว (%TEMP%)",
        description: "ย้ายรายการใน %TEMP% ที่ไม่ได้แตะเกิน 24 ชั่วโมงไปถังขยะ (กู้คืนได้ ข้ามไฟล์ที่กำลังใช้งาน) — พื้นที่จะคืนเมื่อล้างถังขยะ",
        kind: CleanupKind::TempFiles,
        destructive: true,
        permanent: false,
    },
    CleanupSpec {
        id: "empty-recycle-bin",
        title: "ล้างถังขยะ (Recycle Bin)",
        description: "รัน `Clear-RecycleBin -Force` — เป็นการลบถาวรอย่างเดียวที่ DiskWhy ทำ: ไฟล์ทั้งหมดในถังขยะของทุกไดรฟ์จะกู้คืนไม่ได้อีก",
        kind: CleanupKind::EmptyRecycleBin,
        destructive: true,
        permanent: true,
    },
    CleanupSpec {
        id: "open-storage-sense",
        title: "เปิด Storage Sense",
        description: "เปิด Settings › System › Storage ให้ผู้ใช้เลือกทำความสะอาดเอง",
        kind: CleanupKind::OpenTool("storage-sense"),
        destructive: false,
        permanent: false,
    },
    CleanupSpec {
        id: "open-disk-cleanup",
        title: "เปิด Disk Cleanup",
        description: "เปิด cleanmgr.exe ให้ผู้ใช้เลือกทำความสะอาดเอง (รวม Clean up system files)",
        kind: CleanupKind::OpenTool("disk-cleanup"),
        destructive: false,
        permanent: false,
    },
    CleanupSpec {
        id: "open-apps-features",
        title: "เปิด Apps & features",
        description: "เปิด Settings › Apps ให้ผู้ใช้ถอนการติดตั้งโปรแกรมหรือเกมที่ไม่ใช้",
        kind: CleanupKind::OpenTool("apps-features"),
        destructive: false,
        permanent: false,
    },
    CleanupSpec {
        id: "open-recycle-bin",
        title: "เปิดถังขยะ",
        description: "เปิดหน้าต่าง Recycle Bin ให้ผู้ใช้ตรวจดูหรือกู้คืนไฟล์",
        kind: CleanupKind::OpenTool("recycle-bin"),
        destructive: false,
        permanent: false,
    },
];

/// Fixed script for `empty-recycle-bin` (a literal: nothing is interpolated into it).
const EMPTY_RECYCLE_BIN: CommandSpec = cmd(
    "powershell.exe",
    &["-NoProfile", "-NonInteractive", "-Command", "Clear-RecycleBin -Force -ErrorAction SilentlyContinue"],
);

pub fn cleanup_spec(id: &str) -> Option<&'static CleanupSpec> {
    CLEANUP_ACTIONS.iter().find(|a| a.id == id)
}

pub fn cleanup_ids() -> Vec<&'static str> {
    CLEANUP_ACTIONS.iter().map(|a| a.id).collect()
}

/// The exact command line(s) an action runs, for display ("what would this do?").
pub fn describe_commands(spec: &CleanupSpec) -> Vec<String> {
    let show = |c: &CommandSpec| std::iter::once(c.program).chain(c.args.iter().copied()).collect::<Vec<_>>().join(" ");
    match spec.kind {
        CleanupKind::Command(cmds) => cmds.iter().map(show).collect(),
        CleanupKind::EmptyRecycleBin => vec![show(&EMPTY_RECYCLE_BIN)],
        CleanupKind::TempFiles => vec![format!("(move entries older than 24 h in {} to the Recycle Bin)", std::env::temp_dir().display())],
        CleanupKind::OpenTool(t) => system_tool_command(t)
            .map(|(p, a)| vec![std::iter::once(p).chain(a.iter().copied()).collect::<Vec<_>>().join(" ")])
            .unwrap_or_default(),
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CleanupStatus {
    /// Finished successfully.
    Done,
    /// The tool is not installed on this PC (not an error).
    NotInstalled,
    /// Started but returned an error.
    Failed,
    /// Killed after [`CLEANUP_TIMEOUT`].
    Timeout,
    /// A Windows window was opened for the user.
    Opened,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CleanupOutcome {
    pub action: String,
    pub status: CleanupStatus,
    /// Thai summary.
    pub message: String,
    /// The command line that ran (display only).
    pub command: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout_tail: String,
    pub stderr_tail: String,
    /// Free bytes on the system drive before / after.
    pub free_before: Option<u64>,
    pub free_after: Option<u64>,
    /// `free_after - free_before`, never negative.
    pub freed_bytes: u64,
    /// temp-files only: bytes moved to the Recycle Bin (space returns after emptying it).
    pub moved_to_recycle_bytes: u64,
    pub items_moved: u64,
    pub items_skipped: u64,
}

impl CleanupOutcome {
    fn new(action: &str, status: CleanupStatus, message: impl Into<String>) -> Self {
        Self {
            action: action.to_string(),
            status,
            message: message.into(),
            command: None,
            exit_code: None,
            stdout_tail: String::new(),
            stderr_tail: String::new(),
            free_before: None,
            free_after: None,
            freed_bytes: 0,
            moved_to_recycle_bytes: 0,
            items_moved: 0,
            items_skipped: 0,
        }
    }
}

/// Free bytes on the system drive (`%SystemDrive%`, default `C:`).
pub fn system_drive_free() -> Option<u64> {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    report::drive_for(Path::new(&format!("{}\\", drive.trim_end_matches(['\\', '/'])))).map(|d| d.available)
}

/// Run one whitelisted cleanup action. Blocking (up to [`CLEANUP_TIMEOUT`]). Unknown ids are rejected.
pub fn run_cleanup(id: &str) -> Result<CleanupOutcome, String> {
    let spec = cleanup_spec(id.trim()).ok_or_else(|| format!("ไม่รองรับ action นี้: {id:?} (ใช้ได้: {})", cleanup_ids().join(", ")))?;
    let before = system_drive_free();
    let mut out = match spec.kind {
        CleanupKind::Command(cmds) => run_commands(spec.id, cmds),
        CleanupKind::EmptyRecycleBin => run_commands(spec.id, &[EMPTY_RECYCLE_BIN]),
        CleanupKind::TempFiles => run_temp_cleanup(spec.id),
        CleanupKind::OpenTool(tool) => match open_system_tool(tool) {
            Ok(()) => CleanupOutcome::new(spec.id, CleanupStatus::Opened, format!("เปิด {} แล้ว — ให้ผู้ใช้ทำต่อในหน้าต่างนั้น", spec.title)),
            Err(e) => CleanupOutcome::new(spec.id, CleanupStatus::Failed, e),
        },
    };
    if out.status != CleanupStatus::Opened {
        out.free_before = before;
        out.free_after = system_drive_free();
        out.freed_bytes = match (out.free_before, out.free_after) {
            (Some(b), Some(a)) => a.saturating_sub(b),
            _ => 0,
        };
    }
    Ok(out)
}

/* ── Command execution ───────────────────────────────────────────────────── */

struct RawRun {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

/// Exit code of `cmd /C <missing program>`.
const CMD_NOT_FOUND: i32 = 9009;
const OUTPUT_KEEP: usize = 16 * 1024;

/// Last lines of `s`, at most `max_chars` characters.
pub fn tail(s: &str, max_chars: usize) -> String {
    let s = s.trim();
    let lines: Vec<&str> = s.lines().collect();
    let joined = lines[lines.len().saturating_sub(20)..].join("\n");
    let n = joined.chars().count();
    if n <= max_chars { joined } else { joined.chars().skip(n - max_chars).collect() }
}

fn spawn_reader<R: Read + Send + 'static>(mut r: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut keep: Vec<u8> = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n) = r.read(&mut buf) {
            if n == 0 {
                break;
            }
            keep.extend_from_slice(&buf[..n]);
            if keep.len() > OUTPUT_KEEP * 2 {
                keep.drain(..keep.len() - OUTPUT_KEEP);
            }
        }
        keep
    })
}

#[cfg(windows)]
fn no_window(c: &mut Command) {
    use std::os::windows::process::CommandExt;
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
fn no_window(_: &mut Command) {}

fn kill_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let mut k = Command::new("taskkill.exe");
        k.args(["/PID", &child.id().to_string(), "/T", "/F"]).stdout(Stdio::null()).stderr(Stdio::null());
        no_window(&mut k);
        let _ = k.status();
    }
    let _ = child.kill();
}

fn run_with_timeout(program: &str, args: &[&str], timeout: Duration) -> std::io::Result<RawRun> {
    let mut c = Command::new(program);
    c.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    no_window(&mut c);
    let mut child = c.spawn()?;
    let out = child.stdout.take().map(spawn_reader);
    let err = child.stderr.take().map(spawn_reader);
    let started = Instant::now();
    let (code, timed_out) = loop {
        match child.try_wait()? {
            Some(status) => break (status.code(), false),
            None if started.elapsed() >= timeout => {
                kill_tree(&mut child);
                let _ = child.wait();
                break (None, true);
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let text = |h: Option<std::thread::JoinHandle<Vec<u8>>>| {
        h.and_then(|h| h.join().ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
    };
    Ok(RawRun { code, stdout: text(out), stderr: text(err), timed_out })
}

/// Did the shell / launcher say the program does not exist?
fn looks_missing(run: &RawRun) -> bool {
    let all = format!("{}\n{}", run.stdout, run.stderr).to_lowercase();
    run.code == Some(CMD_NOT_FOUND)
        || all.contains("is not recognized as an internal or external command")
        || all.contains("was not found; run without arguments")
        || all.contains("no installed python")
}

fn run_commands(id: &str, cmds: &[CommandSpec]) -> CleanupOutcome {
    run_commands_with(id, cmds, CLEANUP_TIMEOUT)
}

fn run_commands_with(id: &str, cmds: &[CommandSpec], timeout: Duration) -> CleanupOutcome {
    let mut last_missing: Option<String> = None;
    for c in cmds {
        let shown = std::iter::once(c.program).chain(c.args.iter().copied()).collect::<Vec<_>>().join(" ");
        let run = match run_with_timeout(c.program, c.args, timeout) {
            Ok(r) => r,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                last_missing = Some(shown);
                continue;
            }
            Err(e) => {
                let mut o = CleanupOutcome::new(id, CleanupStatus::Failed, format!("เริ่มคำสั่งไม่สำเร็จ: {e}"));
                o.command = Some(shown);
                return o;
            }
        };
        if looks_missing(&run) {
            last_missing = Some(shown);
            continue;
        }
        let mut o = CleanupOutcome::new(id, CleanupStatus::Done, "");
        o.command = Some(shown.clone());
        o.exit_code = run.code;
        o.stdout_tail = tail(&run.stdout, 1500);
        o.stderr_tail = tail(&run.stderr, 1500);
        if run.timed_out {
            o.status = CleanupStatus::Timeout;
            o.message = format!("คำสั่งใช้เวลาเกิน {} นาที จึงถูกยกเลิก: {shown}", timeout.as_secs() / 60);
        } else if run.code == Some(0) {
            o.message = format!("สำเร็จ: {shown}");
        } else {
            o.status = CleanupStatus::Failed;
            o.message = format!("คำสั่งจบด้วยรหัส {}: {shown}", run.code.map_or("?".into(), |c| c.to_string()));
        }
        return o;
    }
    let mut o = CleanupOutcome::new(id, CleanupStatus::NotInstalled, "ไม่พบเครื่องมือนี้ในเครื่อง (ยังไม่ได้ติดตั้งหรือไม่อยู่ใน PATH) — จึงไม่มีอะไรให้ล้าง");
    o.command = last_missing;
    o
}

/* ── temp-files ──────────────────────────────────────────────────────────── */

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempEntry {
    pub path: PathBuf,
    pub bytes: u64,
    pub is_dir: bool,
}

/// (bytes, newest modification time) of a file or of everything inside a directory. Symlinks are not followed.
fn measure(path: &Path, budget: &mut u32) -> (u64, SystemTime) {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return (0, SystemTime::now()) };
    let mtime = meta.modified().unwrap_or_else(|_| SystemTime::now());
    if meta.file_type().is_symlink() {
        return (0, mtime);
    }
    if !meta.is_dir() {
        return (meta.len(), mtime);
    }
    let (mut bytes, mut newest) = (0u64, mtime);
    let Ok(rd) = std::fs::read_dir(path) else { return (0, SystemTime::now()) }; // unreadable: treat as "in use"
    for e in rd.flatten() {
        if *budget == 0 {
            return (bytes, SystemTime::now()); // too big to judge cheaply: treat as recent, leave it alone
        }
        *budget -= 1;
        let (b, m) = measure(&e.path(), budget);
        bytes += b;
        newest = newest.max(m);
    }
    (bytes, newest)
}

/// Direct children of `dir` whose newest modification (anywhere inside) is before `cutoff`.
pub fn select_old_temp_entries(dir: &Path, cutoff: SystemTime) -> Vec<TempEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let path = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let mut budget = 50_000u32;
        let (bytes, newest) = measure(&path, &mut budget);
        if newest < cutoff {
            out.push(TempEntry { path, bytes, is_dir: ft.is_dir() });
        }
    }
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    out
}

/// Move old entries of `dir` through `recycle`; returns (moved, skipped, moved bytes). Stops at `deadline`.
pub fn clean_temp_dir(
    dir: &Path,
    min_age: Duration,
    deadline: Instant,
    recycle: &dyn Fn(&Path) -> Result<(), String>,
) -> (u64, u64, u64) {
    let cutoff = SystemTime::now().checked_sub(min_age).unwrap_or(SystemTime::UNIX_EPOCH);
    let (mut moved, mut skipped, mut bytes) = (0u64, 0u64, 0u64);
    for entry in select_old_temp_entries(dir, cutoff) {
        if Instant::now() >= deadline {
            skipped += 1;
            continue;
        }
        match recycle(&entry.path) {
            Ok(()) => {
                moved += 1;
                bytes += entry.bytes;
            }
            Err(_) => skipped += 1, // locked / in use / vanished: skip silently
        }
    }
    (moved, skipped, bytes)
}

fn run_temp_cleanup(id: &str) -> CleanupOutcome {
    let dir = std::env::temp_dir();
    if safety::is_protected(&dir) || dir.components().count() < 4 {
        return CleanupOutcome::new(id, CleanupStatus::Failed, format!("โฟลเดอร์ชั่วคราวอยู่ในตำแหน่งที่ไม่ปลอดภัยที่จะล้าง: {}", dir.display()));
    }
    let (moved, skipped, bytes) = clean_temp_dir(&dir, TEMP_MIN_AGE, Instant::now() + CLEANUP_TIMEOUT, &|p| {
        // Stay strictly inside the temp folder.
        if p.parent() != Some(dir.as_path()) {
            return Err("outside temp".into());
        }
        safety::recycle(p)
    });
    let mut o = CleanupOutcome::new(
        id,
        CleanupStatus::Done,
        format!("ย้าย {moved} รายการจาก %TEMP% ไปถังขยะแล้ว (ข้าม {skipped} รายการที่ใช้งานอยู่หรือใหม่เกินไป) — พื้นที่จะคืนเมื่อล้างถังขยะ"),
    );
    o.command = Some(format!("move to Recycle Bin: entries older than 24 h in {}", dir.display()));
    o.items_moved = moved;
    o.items_skipped = skipped;
    o.moved_to_recycle_bytes = bytes;
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_only() {
        for t in SYSTEM_TOOLS {
            assert!(system_tool_command(t).is_some());
        }
        for bad in ["", "cmd", "storage-sense && del", "../x", "DISK-CLEANUP"] {
            assert!(system_tool_command(bad).is_none(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn cleanup_table_is_consistent() {
        let ids = cleanup_ids();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "ids are unique");
        for want in [
            "npm-cache", "pnpm-store", "yarn-cache", "pip-cache", "nuget-cache", "wsl-shutdown", "temp-files", "empty-recycle-bin",
            "open-storage-sense", "open-disk-cleanup", "open-apps-features", "open-recycle-bin",
        ] {
            assert!(cleanup_spec(want).is_some(), "{want} missing");
        }
        for a in CLEANUP_ACTIONS {
            assert!(!a.title.is_empty() && !a.description.is_empty());
            match a.kind {
                CleanupKind::Command(cmds) => {
                    assert!(!cmds.is_empty());
                    for c in cmds {
                        assert!(c.program.ends_with(".exe"), "{} must name an exe", c.program);
                    }
                }
                CleanupKind::OpenTool(t) => assert!(system_tool_command(t).is_some()),
                _ => {}
            }
            if a.permanent {
                assert!(a.destructive);
            }
            assert!(!describe_commands(a).is_empty());
        }
    }

    #[test]
    fn exact_command_lines() {
        let line = |id: &str| describe_commands(cleanup_spec(id).unwrap());
        assert_eq!(line("npm-cache"), ["cmd.exe /C npm cache clean --force"]);
        assert_eq!(line("pnpm-store"), ["cmd.exe /C pnpm store prune"]);
        assert_eq!(line("yarn-cache"), ["cmd.exe /C yarn cache clean"]);
        assert_eq!(line("pip-cache"), ["python.exe -m pip cache purge", "py.exe -m pip cache purge"]);
        assert_eq!(line("nuget-cache"), ["dotnet.exe nuget locals all --clear"]);
        assert_eq!(line("wsl-shutdown"), ["wsl.exe --shutdown"]);
        assert_eq!(
            line("empty-recycle-bin"),
            ["powershell.exe -NoProfile -NonInteractive -Command Clear-RecycleBin -Force -ErrorAction SilentlyContinue"]
        );
        // Only the Recycle Bin and the package caches are permanent; temp-files goes through the Recycle Bin.
        assert!(!cleanup_spec("temp-files").unwrap().permanent);
        assert!(!cleanup_spec("open-recycle-bin").unwrap().destructive);
    }

    #[test]
    fn unknown_actions_are_rejected_without_running_anything() {
        for bad in ["", "rm -rf", "npm-cache && calc", "NPM-CACHE", "format", "../npm-cache"] {
            let err = run_cleanup(bad).unwrap_err();
            assert!(err.contains("ไม่รองรับ"), "{bad:?}: {err}");
        }
    }

    #[test]
    fn tail_keeps_last_lines() {
        let text = (1..=50).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let t = tail(&text, 10_000);
        assert!(t.starts_with("line 31") && t.ends_with("line 50"));
        assert_eq!(tail(&text, 7), "line 50");
        assert_eq!(tail("  \n", 100), "");
    }

    #[test]
    fn old_temp_entries_are_selected_by_newest_mtime_inside() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        let old = SystemTime::now() - Duration::from_secs(3 * 86_400);
        let set = |p: &Path, t: SystemTime| {
            let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
            f.set_modified(t).unwrap();
        };
        std::fs::write(r.join("old.tmp"), vec![0u8; 100]).unwrap();
        set(&r.join("old.tmp"), old);
        std::fs::write(r.join("new.tmp"), vec![0u8; 100]).unwrap();
        std::fs::create_dir_all(r.join("stale_dir/inner")).unwrap();
        std::fs::write(r.join("stale_dir/inner/a.bin"), vec![0u8; 300]).unwrap();
        set(&r.join("stale_dir/inner/a.bin"), old);
        // a directory that looks old but holds a fresh file must be kept
        std::fs::create_dir_all(r.join("active_dir")).unwrap();
        std::fs::write(r.join("active_dir/fresh.log"), b"x").unwrap();

        let cutoff = SystemTime::now() - TEMP_MIN_AGE;
        // directories' own mtime is "now" (just created), so they are only old if they are backdated
        let picked = select_old_temp_entries(r, cutoff);
        let names: Vec<_> = picked.iter().map(|e| e.path.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, ["old.tmp"], "dirs created just now and fresh files are kept: {names:?}");
        assert_eq!(picked[0].bytes, 100);
    }

    #[test]
    fn temp_cleanup_uses_the_injected_recycler_and_skips_failures() {
        let dir = tempfile::tempdir().unwrap();
        let old = SystemTime::now() - Duration::from_secs(5 * 86_400);
        for n in ["a.tmp", "locked.tmp", "c.tmp"] {
            let p = dir.path().join(n);
            std::fs::write(&p, vec![1u8; 10]).unwrap();
            std::fs::OpenOptions::new().write(true).open(&p).unwrap().set_modified(old).unwrap();
        }
        let seen = std::sync::Mutex::new(Vec::new());
        let (moved, skipped, bytes) = clean_temp_dir(dir.path(), TEMP_MIN_AGE, Instant::now() + Duration::from_secs(30), &|p| {
            seen.lock().unwrap().push(p.to_path_buf());
            if p.ends_with("locked.tmp") { Err("in use".into()) } else { Ok(()) }
        });
        assert_eq!((moved, skipped, bytes), (2, 1, 20));
        assert_eq!(seen.lock().unwrap().len(), 3);
        // an expired deadline processes nothing
        let (m, s, _) = clean_temp_dir(dir.path(), TEMP_MIN_AGE, Instant::now() - Duration::from_secs(1), &|_| Ok(()));
        assert_eq!((m, s), (0, 3));
    }

    #[cfg(windows)]
    #[test]
    fn command_runner_captures_output_and_handles_missing_programs() {
        let ok = run_commands_with("t", &[cmd("cmd.exe", &["/C", "echo", "hello-diskwhy"])], Duration::from_secs(20));
        assert_eq!(ok.status, CleanupStatus::Done);
        assert!(ok.stdout_tail.contains("hello-diskwhy"), "{ok:?}");

        let failed = run_commands_with("t", &[cmd("cmd.exe", &["/C", "exit", "3"])], Duration::from_secs(20));
        assert_eq!(failed.status, CleanupStatus::Failed);
        assert_eq!(failed.exit_code, Some(3));

        let missing_exe = run_commands_with("t", &[cmd("definitely-not-a-real-tool-xyz.exe", &[])], Duration::from_secs(20));
        assert_eq!(missing_exe.status, CleanupStatus::NotInstalled);

        let missing_cmd = run_commands_with("t", &[cmd("cmd.exe", &["/C", "definitely-not-a-real-tool-xyz", "--x"])], Duration::from_secs(20));
        assert_eq!(missing_cmd.status, CleanupStatus::NotInstalled, "{missing_cmd:?}");

        // fallback: first program missing, second one works
        let fb = run_commands_with(
            "t",
            &[cmd("definitely-not-a-real-tool-xyz.exe", &[]), cmd("cmd.exe", &["/C", "echo", "fallback"])],
            Duration::from_secs(20),
        );
        assert_eq!(fb.status, CleanupStatus::Done);
        assert!(fb.stdout_tail.contains("fallback"));
    }

    #[cfg(windows)]
    #[test]
    fn command_runner_times_out() {
        let t = Instant::now();
        let o = run_commands_with("t", &[cmd("cmd.exe", &["/C", "ping", "-n", "30", "127.0.0.1"])], Duration::from_secs(1));
        assert_eq!(o.status, CleanupStatus::Timeout);
        assert!(t.elapsed() < Duration::from_secs(15));
    }
}
