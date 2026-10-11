//! The MCP server (stdio). Thin layer over [`Engine`]: tool/prompt metadata + argument parsing only.
//! stdout carries the protocol and nothing else; diagnostics go to stderr.

use crate::engine::{CLEANUP_WAIT, Engine};
use crate::policy;
use crate::views::Reply;
use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, GetPromptResult, Implementation, PromptMessage, Role, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, ServiceExt, prompt, prompt_handler, prompt_router, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;

/* ── Arguments ───────────────────────────────────────────────────────────── */

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScanArgs {
    /// Absolute path of the drive or folder to scan, e.g. "C:\\" or "C:\\Users\\me". Must be an existing directory.
    pub path: String,
    /// Seconds to wait for the scan to finish before answering (default 45, max 55). If it is still running the
    /// answer has status "running": call scan_status repeatedly until it is "done".
    pub wait_seconds: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReportArgs {
    /// Maximum number of findings to return (default 20).
    pub max_findings: Option<u64>,
    /// Maximum number of locations listed per finding (default 8).
    pub max_locations: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BrowseArgs {
    /// Absolute path of a folder (or file) inside the last scan. Defaults to the scan root.
    pub path: Option<String>,
    /// Maximum number of children to list (default 40, max 200); the rest is summarised in one "rest" entry.
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LargestArgs {
    /// Number of files to return (default 30, max 200).
    pub limit: Option<u64>,
    /// Only files at least this many MiB.
    pub min_size_mb: Option<f64>,
    /// Only this category: video, image, audio, archive, disk-image, installer, document, system, temp-log, code, game, database, other.
    pub category: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveArgs {
    /// Absolute paths of files/folders to move to the Recycle Bin (1 to 50). Use exactly the paths DiskWhy returned.
    pub paths: Vec<String>,
    /// Why these items are being removed (shown in the result and kept for the record).
    pub reason: Option<String>,
}

fn cleanup_action_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let ids = diskwhy_core::actions::cleanup_ids();
    schemars::json_schema!({ "type": "string", "enum": ids })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CleanupArgs {
    /// One of the whitelisted actions (anything else is rejected).
    #[schemars(schema_with = "cleanup_action_schema")]
    pub action: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DiagnoseArgs {
    /// Drive or folder to analyse, e.g. "C:\\". Defaults to the system drive.
    pub drive: Option<String>,
}

/* ── Server ──────────────────────────────────────────────────────────────── */

#[derive(Clone)]
pub struct DiskWhyServer {
    engine: Arc<Engine>,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

/// Text for the human + the same data as `structuredContent`.
pub fn to_result(r: Reply) -> CallToolResult {
    let content = vec![ContentBlock::text(r.text)];
    let mut res = if r.is_error { CallToolResult::error(content) } else { CallToolResult::success(content) };
    res.structured_content = Some(r.data);
    res
}

/// Run blocking engine work off the async runtime threads.
async fn blocking(f: impl FnOnce() -> Reply + Send + 'static) -> CallToolResult {
    to_result(tokio::task::spawn_blocking(f).await.unwrap_or_else(|e| Reply::err(format!("เกิดข้อผิดพลาดภายใน: {e}"))))
}

impl DiskWhyServer {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self { engine, tool_router: Self::tool_router(), prompt_router: Self::prompt_router() }
    }
}

#[tool_router]
impl DiskWhyServer {
    #[tool(
        description = "List the PC's drives with total/free/used space and used %. Drives that are 90% or more full are flagged (nearlyFull). Start here.",
        annotations(title = "List drives", read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn list_drives(&self) -> CallToolResult {
        blocking(crate::views::drives_reply).await
    }

    #[tool(
        description = "Scan a drive or folder (read-only) and keep the result in memory for the other tools. A full C:\\ scan takes 1-3 minutes, so the scan runs in the background: this call waits up to wait_seconds (default 45, max 55) and returns status \"running\" (files/bytes so far) or \"done\" (total size, files, dirs, unreadable folders, cloud-only bytes, top 5 findings). While the status is \"running\" call scan_status again until it is \"done\" - do not start another scan. Scanning the same path again joins the running scan; a different path cancels it. Then call disk_report and find_unused.",
        annotations(title = "Scan a drive or folder", read_only_hint = true, open_world_hint = false)
    )]
    async fn scan(&self, Parameters(args): Parameters<ScanArgs>) -> CallToolResult {
        to_result(self.engine.scan(&args.path, args.wait_seconds).await)
    }

    #[tool(
        description = "Progress of the background scan without waiting: status \"running\" (files, bytes, current folder, elapsed), \"done\" (same summary as scan), \"cancelled\" or \"idle\" (no scan yet). Call repeatedly while a scan is running.",
        annotations(title = "Scan status", read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn scan_status(&self) -> CallToolResult {
        let engine = self.engine.clone();
        blocking(move || engine.scan_status()).await
    }

    #[tool(
        description = "Cancel the running background scan (the previous finished scan result, if any, is kept).",
        annotations(title = "Cancel scan", read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn cancel_scan(&self) -> CallToolResult {
        to_result(self.engine.cancel_scan())
    }

    #[tool(
        description = "Full report of the last finished scan: every finding (known space hogs plus stale-large-files, stale-projects, old-installers, orphaned-appdata) with id, label, safety (safe = regenerable, review = personal data/apps, system = managed by Windows), size, count, advice and the largest locations (path, size, last used, note); the file-category breakdown; the drive's free space; and the total that is safe to reclaim right now. Needs a finished scan.",
        annotations(title = "Disk report", read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn disk_report(&self, Parameters(args): Parameters<ReportArgs>) -> CallToolResult {
        let engine = self.engine.clone();
        blocking(move || engine.disk_report(args.max_findings, args.max_locations)).await
    }

    #[tool(
        description = "Only the unused/stale data from the last scan, with up to 30 locations each: big files untouched for over a year (stale-large-files), developer projects untouched for 90+ days with their rebuildable folders (stale-projects), old installers/ISOs/archives in Downloads (old-installers) and AppData of programs that look uninstalled (orphaned-appdata). Each location has its age. Needs a finished scan.",
        annotations(title = "Find unused data", read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn find_unused(&self) -> CallToolResult {
        let engine = self.engine.clone();
        blocking(move || engine.find_unused()).await
    }

    #[tool(
        description = "Look inside a folder of the last scan: its children sorted by size with kind, size, % of the parent, file count and last-modified age. Without a path it shows the scan root. Drill down with the returned child paths. Files smaller than 1 MB are grouped in one entry.",
        annotations(title = "Browse scanned folder", read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn browse(&self, Parameters(args): Parameters<BrowseArgs>) -> CallToolResult {
        let engine = self.engine.clone();
        blocking(move || engine.browse(args.path.as_deref(), args.limit)).await
    }

    #[tool(
        description = "The largest files in the last scan (files of 1 MB or more), optionally filtered by minimum size in MiB and/or category. Default 30 files, max 200.",
        annotations(title = "Largest files", read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn largest_files(&self, Parameters(args): Parameters<LargestArgs>) -> CallToolResult {
        let engine = self.engine.clone();
        blocking(move || engine.largest_files(args.limit, args.min_size_mb, args.category.as_deref())).await
    }

    #[tool(
        description = "Move files/folders to the Windows Recycle Bin (recoverable; never a hard delete). IMPORTANT: first show the exact list to the user and get an explicit yes; use only paths returned by DiskWhy. System locations (Windows, Program Files, drive roots, pagefile/hiberfil...) are refused, and when a scan exists the paths must lie inside the scanned root. Items stay in the Recycle Bin, so free space only increases after the bin is emptied (run_cleanup empty-recycle-bin, which is permanent). Returns a per-path result and the total moved.",
        annotations(title = "Move to Recycle Bin", read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn move_to_recycle_bin(&self, Parameters(args): Parameters<MoveArgs>) -> CallToolResult {
        let engine = self.engine.clone();
        blocking(move || engine.move_to_recycle_bin(&args.paths, args.reason.as_deref())).await
    }

    #[tool(
        description = "Run ONE whitelisted cleanup action (fixed commands, no shell strings). Confirm with the user first. Actions (cache cleanups delete re-downloadable caches permanently): npm-cache (npm cache clean --force), pnpm-store (pnpm store prune), yarn-cache (yarn cache clean), pip-cache (python -m pip cache purge), nuget-cache (dotnet nuget locals all --clear), wsl-shutdown (wsl --shutdown; frees nothing itself, needed before compacting a .vhdx and stops running WSL/Docker), temp-files (move %TEMP% entries older than 24 h to the Recycle Bin, skipping in-use files), empty-recycle-bin (PowerShell Clear-RecycleBin -Force: permanently deletes everything in the Recycle Bin on all drives, cannot be undone - ask for a separate explicit confirmation), open-storage-sense / open-disk-cleanup / open-apps-features / open-recycle-bin (open the Windows tool for the user). Reports measured free space before/after on the system drive; a tool that is not installed returns a friendly not-installed result. Long commands: if the answer says status \"running\", call run_cleanup again with the same action to collect the result.",
        annotations(title = "Run a whitelisted cleanup", read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = false)
    )]
    async fn run_cleanup(&self, Parameters(args): Parameters<CleanupArgs>) -> CallToolResult {
        to_result(self.engine.run_cleanup(&args.action, CLEANUP_WAIT).await)
    }

    #[tool(
        description = "The exact elevated commands the USER should run themselves in \"Terminal (Admin)\", each with what it frees and its trade-off: DISM component cleanup, powercfg /h off, wsl --shutdown + Optimize-VHD (only for .vhdx files actually found in the last scan), cleanmgr /sageset:1 then /sagerun:1. DiskWhy never executes these.",
        annotations(title = "Admin commands for the user", read_only_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn admin_commands(&self) -> CallToolResult {
        let engine = self.engine.clone();
        blocking(move || engine.admin_commands()).await
    }
}

#[prompt_router]
impl DiskWhyServer {
    #[prompt(name = "diagnose-disk", description = policy::DIAGNOSE_DESCRIPTION)]
    async fn diagnose_disk(&self, Parameters(args): Parameters<DiagnoseArgs>) -> GetPromptResult {
        GetPromptResult::new(vec![PromptMessage::new_text(Role::User, policy::diagnose_prompt(args.drive.as_deref()))])
            .with_description(policy::DIAGNOSE_DESCRIPTION)
    }

    #[prompt(name = "cleanup-quick-wins", description = policy::QUICK_WINS_DESCRIPTION)]
    async fn cleanup_quick_wins(&self) -> GetPromptResult {
        GetPromptResult::new(vec![PromptMessage::new_text(Role::User, policy::quick_wins_prompt())]).with_description(policy::QUICK_WINS_DESCRIPTION)
    }
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for DiskWhyServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_prompts().build())
            .with_server_info(Implementation::new("diskwhy", env!("CARGO_PKG_VERSION")).with_title("DiskWhy"))
            .with_instructions(policy::INSTRUCTIONS)
    }
}

/// Run the MCP server on stdin/stdout until the client disconnects.
pub fn run_stdio() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime: {e}"))?;
    runtime.block_on(async {
        eprintln!("diskwhy-cli {} MCP server (stdio) ready", env!("CARGO_PKG_VERSION"));
        let service = DiskWhyServer::new(Engine::new()).serve(rmcp::transport::stdio()).await.map_err(|e| format!("MCP initialize failed: {e}"))?;
        service.waiting().await.map_err(|e| format!("MCP server error: {e}"))?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<rmcp::model::Tool> {
        DiskWhyServer::tool_router().list_all()
    }

    #[test]
    fn exact_tool_names_and_annotations() {
        let tools = tools();
        let mut names: Vec<_> = tools.iter().map(|t| t.name.to_string()).collect();
        names.sort();
        let mut want = vec![
            "list_drives", "scan", "scan_status", "cancel_scan", "disk_report", "find_unused", "browse", "largest_files", "move_to_recycle_bin",
            "run_cleanup", "admin_commands",
        ];
        want.sort();
        assert_eq!(names, want);
        for t in &tools {
            let a = t.annotations.as_ref().unwrap_or_else(|| panic!("{} has no annotations", t.name));
            assert_eq!(a.open_world_hint, Some(false), "{}", t.name);
            assert!(a.read_only_hint.is_some(), "{}", t.name);
            assert!(t.description.as_deref().is_some_and(|d| d.len() > 40), "{} needs a real description", t.name);
            let destructive = matches!(&*t.name, "move_to_recycle_bin" | "run_cleanup");
            assert_eq!(a.read_only_hint == Some(false) && a.destructive_hint == Some(true), destructive, "{}", t.name);
            if destructive {
                assert_eq!(a.idempotent_hint, Some(false));
            }
        }
        let by = |n: &str| tools.iter().find(|t| &*t.name == n).unwrap();
        for ro in ["list_drives", "scan", "scan_status", "disk_report", "find_unused", "browse", "largest_files", "admin_commands"] {
            assert_eq!(by(ro).annotations.as_ref().unwrap().read_only_hint, Some(true), "{ro}");
        }
        assert_eq!(by("cancel_scan").annotations.as_ref().unwrap().destructive_hint, Some(false));
        let rec = by("move_to_recycle_bin").description.clone().unwrap().to_lowercase();
        assert!(rec.contains("recycle bin") && rec.contains("explicit yes") && rec.contains("recoverable"));
        let cl = by("run_cleanup").description.clone().unwrap();
        assert!(cl.contains("permanently deletes everything in the Recycle Bin") && cl.contains("re-downloadable caches permanently"));
    }

    #[test]
    fn input_schemas() {
        let tools = tools();
        let schema = |n: &str| serde_json::to_value(&*tools.iter().find(|t| &*t.name == n).unwrap().input_schema).unwrap();
        assert_eq!(schema("scan")["required"], serde_json::json!(["path"]));
        assert_eq!(schema("move_to_recycle_bin")["required"], serde_json::json!(["paths"]));
        let actions = schema("run_cleanup")["properties"]["action"]["enum"].clone();
        assert_eq!(actions.as_array().unwrap().len(), diskwhy_core::actions::CLEANUP_ACTIONS.len());
        assert!(actions.as_array().unwrap().contains(&serde_json::json!("empty-recycle-bin")));
    }

    #[test]
    fn prompts_are_registered() {
        let prompts = DiskWhyServer::prompt_router().list_all();
        let names: Vec<_> = prompts.iter().map(|p| p.name.to_string()).collect();
        assert!(names.contains(&"diagnose-disk".to_string()) && names.contains(&"cleanup-quick-wins".to_string()), "{names:?}");
        let d = prompts.iter().find(|p| p.name == "diagnose-disk").unwrap();
        let args = d.arguments.clone().unwrap_or_default();
        assert!(args.iter().any(|a| a.name == "drive" && a.required != Some(true)));
    }

    #[test]
    fn server_info_and_instructions() {
        let s = DiskWhyServer::new(Engine::new());
        let info = s.get_info();
        assert_eq!(info.server_info.name, "diskwhy");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.capabilities.tools.is_some() && info.capabilities.prompts.is_some());
        assert!(info.instructions.as_deref().is_some_and(|i| i.contains("move_to_recycle_bin")));
    }

    #[test]
    fn reply_conversion_carries_text_structured_content_and_error_flag() {
        let ok = to_result(Reply::ok("hello", serde_json::json!({ "a": 1 })));
        assert_eq!(ok.is_error, Some(false));
        assert_eq!(ok.structured_content, Some(serde_json::json!({ "a": 1 })));
        assert_eq!(ok.content.len(), 1);
        let err = to_result(Reply::err("bad"));
        assert_eq!(err.is_error, Some(true));
        assert_eq!(err.structured_content.unwrap()["error"], "bad");
    }
}
