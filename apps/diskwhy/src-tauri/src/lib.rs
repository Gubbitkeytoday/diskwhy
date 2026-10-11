mod mcp_setup;

use diskwhy_core::insights::{self, NodeView};
use diskwhy_core::report::{self, DriveInfo, Report, ScanResult, ScanStats};
use diskwhy_core::scan::{self, Progress};
use diskwhy_core::{actions, safety};
use mcp_setup::{Env, McpInfo, McpTarget};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
struct AppState {
    result: Mutex<Option<ScanResult>>,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    seq: AtomicU64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    scan_id: u64,
    files: u64,
    dirs: u64,
    bytes: u64,
    errors: u64,
    current: String,
    elapsed_ms: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DoneEvent {
    scan_id: u64,
    cancelled: bool,
    stats: ScanStats,
}

#[tauri::command]
fn list_drives() -> Vec<DriveInfo> {
    report::drives()
}

/// Start a scan in the background. Progress arrives as `scan-progress` events, completion as `scan-done`.
#[tauri::command]
fn start_scan(path: String, app: AppHandle, state: State<'_, AppState>) -> Result<u64, String> {
    let root_path = PathBuf::from(&path);
    if !root_path.is_dir() {
        return Err(format!("ไม่พบโฟลเดอร์: {path}"));
    }
    // Cancel any scan already running.
    if let Some(prev) = state.cancel.lock().map_err(|e| e.to_string())?.take() {
        prev.store(true, Relaxed);
    }
    let scan_id = state.seq.fetch_add(1, Relaxed) + 1;
    let cancel = Arc::new(AtomicBool::new(false));
    *state.cancel.lock().map_err(|e| e.to_string())? = Some(cancel.clone());

    std::thread::Builder::new()
        .name(format!("scan-{scan_id}"))
        .spawn(move || {
            let started = Instant::now();
            let progress = Arc::new(Progress::default());
            let finished = Arc::new(AtomicBool::new(false));

            // Progress ticker: ~8 updates/second keeps the UI live without flooding IPC.
            let ticker = {
                let (app, progress, finished, cancel) = (app.clone(), progress.clone(), finished.clone(), cancel.clone());
                std::thread::spawn(move || {
                    while !finished.load(Relaxed) && !cancel.load(Relaxed) {
                        let current = progress.current.lock().map(|c| c.clone()).unwrap_or_default();
                        let _ = app.emit(
                            "scan-progress",
                            ProgressEvent {
                                scan_id,
                                files: progress.files.load(Relaxed),
                                dirs: progress.dirs.load(Relaxed),
                                bytes: progress.bytes.load(Relaxed),
                                errors: progress.errors.load(Relaxed),
                                current,
                                elapsed_ms: started.elapsed().as_millis() as u64,
                            },
                        );
                        std::thread::sleep(Duration::from_millis(120));
                    }
                })
            };

            let result = report::run_scan(&root_path, &progress, &cancel);
            finished.store(true, Relaxed);
            let _ = ticker.join();

            let cancelled = result.is_none();
            let stats = result.as_ref().map(|r| r.stats.clone()).unwrap_or_default();
            if let Some(r) = result
                && let Ok(mut slot) = app.state::<AppState>().result.lock()
            {
                *slot = Some(r);
            }
            let _ = app.emit("scan-done", DoneEvent { scan_id, cancelled, stats });
        })
        .map_err(|e| e.to_string())?;
    Ok(scan_id)
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppState>) {
    if let Ok(mut c) = state.cancel.lock()
        && let Some(flag) = c.take()
    {
        flag.store(true, Relaxed);
    }
}

#[tauri::command]
fn get_report(state: State<'_, AppState>) -> Result<Report, String> {
    let guard = state.result.lock().map_err(|e| e.to_string())?;
    let r = guard.as_ref().ok_or("ยังไม่มีผลการสแกน")?;
    Ok(report::build_report(r, 200))
}

#[tauri::command]
fn get_node(ids: Vec<u32>, state: State<'_, AppState>) -> Result<NodeView, String> {
    let guard = state.result.lock().map_err(|e| e.to_string())?;
    let r = guard.as_ref().ok_or("ยังไม่มีผลการสแกน")?;
    insights::node_view(&r.root, &r.root_path, &ids, 300).ok_or_else(|| "ไม่พบโฟลเดอร์นี้ในผลสแกน".into())
}

/// Move a file/folder to the Recycle Bin (recoverable), then update the in-memory tree.
#[tauri::command]
fn move_to_trash(path: String, dir_ids: Vec<u32>, file_name: Option<String>, state: State<'_, AppState>) -> Result<u64, String> {
    let target = PathBuf::from(&path);
    if safety::is_protected(&target) {
        return Err(safety::PROTECTED_MESSAGE.into());
    }
    let mut guard = state.result.lock().map_err(|e| e.to_string())?;
    // The index path must still point at exactly this file/folder; indices shift after earlier removals,
    // and a stale index must never update (or count) the wrong node.
    if let Some(r) = guard.as_ref()
        && !safety::tree_path_matches(&r.root_path, &r.root, &dir_ids, file_name.as_deref(), &path)
    {
        return Err("ผลสแกนเปลี่ยนไปแล้ว (มีการลบรายการอื่นก่อนหน้า) — กรุณาเปิดตำแหน่งนี้ใหม่หรือสแกนอีกครั้ง".into());
    }
    safety::recycle(&target)?;
    let freed = guard
        .as_mut()
        .and_then(|r| scan::remove_from_tree(&mut r.root, &dir_ids, file_name.as_deref()))
        .unwrap_or(0);
    Ok(freed)
}

#[tauri::command]
fn open_system_tool(tool: String) -> Result<(), String> {
    actions::open_system_tool(&tool)
}

/// Where the bundled `diskwhy-cli.exe` is: the installer's resource folder, else beside the running exe, else nowhere.
fn cli_location(app: &AppHandle) -> Option<PathBuf> {
    let resources = app.path().resource_dir().ok();
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from));
    mcp_setup::locate_cli(resources.as_deref(), exe_dir.as_deref())
}

/// Runs blocking process/file work off the UI thread.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// What is installed and what is registered. Read-only.
#[tauri::command]
async fn mcp_info(app: AppHandle) -> Result<McpInfo, String> {
    let cli = cli_location(&app);
    blocking(move || Ok(mcp_setup::collect_info(cli.as_deref(), &Env::from_process()))).await
}

/// Registers DiskWhy's MCP server with Claude Code or Claude Desktop. Only ever called from an explicit button click.
#[tauri::command]
async fn mcp_register(target: McpTarget, app: AppHandle) -> Result<McpInfo, String> {
    let cli = cli_location(&app);
    blocking(move || {
        let env = Env::from_process();
        let note = mcp_setup::register(target, cli.as_deref(), &env)?;
        Ok(McpInfo { note: Some(note), ..mcp_setup::collect_info(cli.as_deref(), &env) })
    })
    .await
}

/// Removes the registration again. Only ever called from an explicit button click.
#[tauri::command]
async fn mcp_unregister(target: McpTarget, app: AppHandle) -> Result<McpInfo, String> {
    let cli = cli_location(&app);
    blocking(move || {
        let env = Env::from_process();
        let note = mcp_setup::unregister(target, &env)?;
        Ok(McpInfo { note: Some(note), ..mcp_setup::collect_info(cli.as_deref(), &env) })
    })
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_drives,
            start_scan,
            cancel_scan,
            get_report,
            get_node,
            move_to_trash,
            open_system_tool,
            mcp_info,
            mcp_register,
            mcp_unregister
        ])
        .run(tauri::generate_context!())
        .expect("error while running DiskWhy");
}
