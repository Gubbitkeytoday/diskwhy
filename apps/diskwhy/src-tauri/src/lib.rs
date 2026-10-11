mod insights;
mod scan;

use insights::{CategoryStat, ExtStat, Hog, NodeView, TopFile};
use scan::{DirNode, Progress, ScanCtx};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

struct ScanResult {
    root_path: PathBuf,
    root: DirNode,
    extensions: HashMap<Box<str>, (u64, u64)>,
    stats: ScanStats,
}

#[derive(Default)]
struct AppState {
    result: Mutex<Option<ScanResult>>,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    seq: AtomicU64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct DriveInfo {
    mount: String,
    name: String,
    file_system: String,
    total: u64,
    available: u64,
    removable: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct ScanStats {
    files: u64,
    dirs: u64,
    bytes: u64,
    errors: u64,
    cloud_bytes: u64,
    elapsed_ms: u64,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    root_path: String,
    total: u64,
    stats: ScanStats,
    categories: Vec<CategoryStat>,
    extensions: Vec<ExtStat>,
    hogs: Vec<Hog>,
    top_files: Vec<TopFile>,
    drive: Option<DriveInfo>,
}

fn drives() -> Vec<DriveInfo> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut out: Vec<DriveInfo> = disks
        .list()
        .iter()
        .map(|d| DriveInfo {
            mount: d.mount_point().to_string_lossy().to_string(),
            name: d.name().to_string_lossy().to_string(),
            file_system: d.file_system().to_string_lossy().to_string(),
            total: d.total_space(),
            available: d.available_space(),
            removable: d.is_removable(),
        })
        .filter(|d| d.total > 0)
        .collect();
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out.dedup_by(|a, b| a.mount == b.mount);
    out
}

#[tauri::command]
fn list_drives() -> Vec<DriveInfo> {
    drives()
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

            let ctx = ScanCtx::new(&progress, &cancel);
            let root = scan::scan_dir(&root_path, root_path.to_string_lossy().into(), &ctx);
            finished.store(true, Relaxed);
            let _ = ticker.join();

            let cancelled = cancel.load(Relaxed);
            let stats = ScanStats {
                files: root.files,
                dirs: root.dirs,
                bytes: root.size,
                errors: progress.errors.load(Relaxed),
                cloud_bytes: progress.cloud_bytes.load(Relaxed),
                elapsed_ms: started.elapsed().as_millis() as u64,
            };
            let state = app.state::<AppState>();
            if !cancelled && let Ok(mut slot) = state.result.lock() {
                let extensions = ctx.extensions.into_inner().unwrap_or_default();
                *slot = Some(ScanResult { root_path, root, extensions, stats: stats.clone() });
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
    let (categories, extensions) = insights::categories(&r.extensions);
    let root_str = r.root_path.to_string_lossy().to_string();
    let drive = drives()
        .into_iter()
        .filter(|d| root_str.to_lowercase().starts_with(&d.mount.to_lowercase()))
        .max_by_key(|d| d.mount.len());
    Ok(Report {
        root_path: root_str,
        total: r.root.size,
        stats: ScanStats { bytes: r.root.size, files: r.root.files, dirs: r.root.dirs, ..r.stats.clone() },
        categories,
        extensions,
        hogs: insights::find_hogs(&r.root, &r.root_path),
        top_files: insights::top_files(&r.root, &r.root_path, 200),
        drive,
    })
}

#[tauri::command]
fn get_node(ids: Vec<u32>, state: State<'_, AppState>) -> Result<NodeView, String> {
    let guard = state.result.lock().map_err(|e| e.to_string())?;
    let r = guard.as_ref().ok_or("ยังไม่มีผลการสแกน")?;
    insights::node_view(&r.root, &r.root_path, &ids, 300).ok_or_else(|| "ไม่พบโฟลเดอร์นี้ในผลสแกน".into())
}

/// Paths the app refuses to touch, even via the Recycle Bin.
fn is_protected(path: &Path) -> bool {
    let p = path.to_string_lossy().to_lowercase().replace('/', "\\");
    let p = p.trim_end_matches('\\');
    let protected_prefixes = ["c:\\windows", "c:\\program files", "c:\\program files (x86)", "c:\\programdata\\microsoft"];
    let protected_names = ["hiberfil.sys", "pagefile.sys", "swapfile.sys", "$recycle.bin", "system volume information", "winsxs"];
    let name = p.rsplit('\\').next().unwrap_or_default();
    p.len() <= 3 // drive roots like "c:"
        || protected_prefixes.iter().any(|pre| p == *pre || p.starts_with(&format!("{pre}\\")))
        || protected_names.contains(&name)
        || p.matches('\\').count() <= 1 && p.starts_with("c:") // top-level folders on the system drive (C:\Users …)
}

/// Move a file/folder to the Recycle Bin (recoverable), then update the in-memory tree.
#[tauri::command]
fn move_to_trash(path: String, dir_ids: Vec<u32>, file_name: Option<String>, state: State<'_, AppState>) -> Result<u64, String> {
    let target = PathBuf::from(&path);
    if is_protected(&target) {
        return Err("ตำแหน่งนี้เป็นไฟล์/โฟลเดอร์ของระบบ — แอปไม่อนุญาตให้ลบเพื่อความปลอดภัย".into());
    }
    trash::delete(&target).map_err(|e| format!("ย้ายไปถังขยะไม่สำเร็จ: {e}"))?;
    let mut guard = state.result.lock().map_err(|e| e.to_string())?;
    let freed = guard
        .as_mut()
        .and_then(|r| scan::remove_from_tree(&mut r.root, &dir_ids, file_name.as_deref()))
        .unwrap_or(0);
    Ok(freed)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![list_drives, start_scan, cancel_scan, get_report, get_node, move_to_trash])
        .run(tauri::generate_context!())
        .expect("error while running DiskWhy");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_system_locations() {
        for p in ["C:\\", "C:\\Windows", "c:\\windows\\System32\\x.dll", "C:\\Program Files\\App", "C:\\Users", "D:\\", "C:\\pagefile.sys", "E:\\$Recycle.Bin"] {
            assert!(is_protected(Path::new(p)), "{p} should be protected");
        }
        for p in ["C:\\Users\\somchai\\Downloads\\setup.exe", "D:\\Videos\\old.mkv", "C:\\Users\\a\\code\\app\\node_modules"] {
            assert!(!is_protected(Path::new(p)), "{p} should be allowed");
        }
    }

    #[test]
    fn lists_at_least_one_drive() {
        assert!(!drives().is_empty());
    }
}
