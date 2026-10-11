//! Scan orchestration + the report model shared by the desktop app and the MCP server/CLI.

use crate::insights::{self, CategoryStat, ExtStat, Hog, TopFile};
use crate::scan::{self, DirNode, Progress, ScanCtx};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::Instant;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    pub mount: String,
    pub name: String,
    pub file_system: String,
    pub total: u64,
    pub available: u64,
    pub removable: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub errors: u64,
    pub cloud_bytes: u64,
    pub elapsed_ms: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub root_path: String,
    pub total: u64,
    pub stats: ScanStats,
    pub categories: Vec<CategoryStat>,
    pub extensions: Vec<ExtStat>,
    pub hogs: Vec<Hog>,
    pub top_files: Vec<TopFile>,
    pub drive: Option<DriveInfo>,
}

/// A finished scan held in memory.
pub struct ScanResult {
    pub root_path: PathBuf,
    pub root: DirNode,
    pub extensions: HashMap<Box<str>, (u64, u64)>,
    pub stats: ScanStats,
}

pub fn drives() -> Vec<DriveInfo> {
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

/// The drive containing `path` (longest matching mount point).
pub fn drive_for(path: &Path) -> Option<DriveInfo> {
    let p = path.to_string_lossy().to_lowercase();
    drives().into_iter().filter(|d| p.starts_with(&d.mount.to_lowercase())).max_by_key(|d| d.mount.len())
}

/// Blocking scan of `root_path`. Progress counters are updated live in `progress`; set `cancel` to stop early.
/// Returns None when cancelled.
pub fn run_scan(root_path: &Path, progress: &Progress, cancel: &AtomicBool) -> Option<ScanResult> {
    let started = Instant::now();
    let ctx = ScanCtx::new(progress, cancel);
    let root = scan::scan_dir(root_path, root_path.to_string_lossy().into(), &ctx);
    if cancel.load(Relaxed) {
        return None;
    }
    let stats = ScanStats {
        files: root.files,
        dirs: root.dirs,
        bytes: root.size,
        errors: progress.errors.load(Relaxed),
        cloud_bytes: progress.cloud_bytes.load(Relaxed),
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    let extensions = ctx.extensions.into_inner().unwrap_or_else(|e| e.into_inner());
    Some(ScanResult { root_path: root_path.to_path_buf(), root, extensions, stats })
}

/// Build the full report (all findings, categories, top files) from a finished scan.
pub fn build_report(r: &ScanResult, top_files: usize) -> Report {
    let (categories, extensions) = insights::categories(&r.extensions);
    Report {
        root_path: r.root_path.to_string_lossy().to_string(),
        total: r.root.size,
        stats: ScanStats { bytes: r.root.size, files: r.root.files, dirs: r.root.dirs, ..r.stats.clone() },
        categories,
        extensions,
        hogs: insights::find_hogs(&r.root, &r.root_path),
        top_files: insights::top_files(&r.root, &r.root_path, top_files),
        drive: drive_for(&r.root_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_at_least_one_drive() {
        assert!(!drives().is_empty());
    }

    #[test]
    fn scan_and_report_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("big.bin"), vec![1u8; 2 * 1024 * 1024]).unwrap();
        let (p, c) = (Progress::default(), AtomicBool::new(false));
        let r = run_scan(dir.path(), &p, &c).expect("not cancelled");
        assert_eq!(r.stats.bytes, 2 * 1024 * 1024);
        let report = build_report(&r, 10);
        assert_eq!(report.top_files.len(), 1);
        assert_eq!(report.total, 2 * 1024 * 1024);
    }

    #[test]
    fn cancelled_scan_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let (p, c) = (Progress::default(), AtomicBool::new(true));
        assert!(run_scan(dir.path(), &p, &c).is_none());
    }
}
