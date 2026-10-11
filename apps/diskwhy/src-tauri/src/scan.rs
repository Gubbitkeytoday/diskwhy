//! Parallel directory scanner.
//!
//! * Fork-join over sub-directories with rayon (work stealing across all cores).
//! * Never follows symlinks or junctions, so nothing is counted twice and scans can't loop.
//! * On Windows `DirEntry::metadata` comes straight from `FindNextFile` data: no extra syscall per file.
//! * Cloud placeholders (OneDrive "online-only") report their full size but use no local space;
//!   they are counted separately instead of inflating the totals.
//! * Memory stays bounded: files below `BIG_FILE` are aggregated per directory instead of stored.
//! * Every file's last-write time (from the same `FindNextFile` data, no extra syscall) feeds
//!   `DirNode::newest`, and well-known project manifests set `DirNode::markers`: both power the
//!   "unused / stale data" detectors in `insights`.

use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::UNIX_EPOCH;

/// Files at least this large are kept individually (top-files list, treemap tiles).
pub const BIG_FILE: u64 = 1024 * 1024;

/* ── Project markers (`DirNode::markers` bit flags) ──────────────────────── */

/// `package.json`
pub const MARKER_JS: u16 = 1 << 0;
/// `Cargo.toml`
pub const MARKER_RUST: u16 = 1 << 1;
/// `pyproject.toml`, `requirements.txt`, `setup.py`
pub const MARKER_PYTHON: u16 = 1 << 2;
/// `build.gradle`, `build.gradle.kts`, `settings.gradle`, `settings.gradle.kts`
pub const MARKER_GRADLE: u16 = 1 << 3;
/// `pom.xml`
pub const MARKER_MAVEN: u16 = 1 << 4;
/// `*.csproj`, `*.fsproj`, `*.sln`
pub const MARKER_DOTNET: u16 = 1 << 5;
/// `go.mod`
pub const MARKER_GO: u16 = 1 << 6;
/// `composer.json`
pub const MARKER_PHP: u16 = 1 << 7;
/// `Gemfile`
pub const MARKER_RUBY: u16 = 1 << 8;
/// `pubspec.yaml`
pub const MARKER_DART: u16 = 1 << 9;

/// Exact (case-insensitive) file names that mark a directory as a developer project.
const MARKER_FILES: &[(&str, u16)] = &[
    ("package.json", MARKER_JS),
    ("Cargo.toml", MARKER_RUST),
    ("pyproject.toml", MARKER_PYTHON),
    ("requirements.txt", MARKER_PYTHON),
    ("setup.py", MARKER_PYTHON),
    ("build.gradle", MARKER_GRADLE),
    ("build.gradle.kts", MARKER_GRADLE),
    ("settings.gradle", MARKER_GRADLE),
    ("settings.gradle.kts", MARKER_GRADLE),
    ("pom.xml", MARKER_MAVEN),
    ("go.mod", MARKER_GO),
    ("composer.json", MARKER_PHP),
    ("Gemfile", MARKER_RUBY),
    ("pubspec.yaml", MARKER_DART),
];

/// Project-marker flag for a file name (0 if it is not a manifest). Allocation-free.
pub fn marker_of(name: &str) -> u16 {
    if let Some(&(_, flag)) = MARKER_FILES.iter().find(|(n, _)| name.eq_ignore_ascii_case(n)) {
        return flag;
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ["csproj", "fsproj", "sln"].iter().any(|e| ext.eq_ignore_ascii_case(e)) => MARKER_DOTNET,
        _ => 0,
    }
}

/// Last-write time as unix seconds; 0 if the platform/file system does not provide one.
fn unix_secs(meta: &fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: Box<str>,
    pub size: u64,
    /// Last-write time, unix seconds (0 if unavailable).
    pub modified: u64,
}

#[derive(Debug, Clone, Default)]
pub struct DirNode {
    pub name: Box<str>,
    pub size: u64,
    pub files: u64,
    pub dirs: u64,
    /// Sub-directories, sorted by size (largest first).
    pub children: Vec<DirNode>,
    /// Files >= BIG_FILE directly in this directory, sorted by size.
    pub big_files: Vec<FileEntry>,
    pub small_count: u64,
    pub small_size: u64,
    /// The directory could not be read (permissions, in use, …).
    pub denied: bool,
    /// Newest last-write time (unix seconds) of ALL files in this subtree, small ones included (0 = none/unknown).
    /// After `remove_from_tree` this is only an upper bound.
    pub newest: u64,
    /// Newest last-write time of the files directly inside this directory.
    pub own_newest: u64,
    /// `MARKER_*` flags for project manifests that sit directly in this directory.
    pub markers: u16,
}

impl DirNode {
    fn named(name: Box<str>) -> Self {
        Self { name, ..Default::default() }
    }
}

#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    pub cloud_bytes: AtomicU64,
    pub current: Mutex<String>,
}

pub struct ScanCtx<'a> {
    pub progress: &'a Progress,
    pub cancel: &'a AtomicBool,
    /// extension → (bytes, count)
    pub extensions: Mutex<HashMap<Box<str>, (u64, u64)>>,
}

impl<'a> ScanCtx<'a> {
    pub fn new(progress: &'a Progress, cancel: &'a AtomicBool) -> Self {
        Self { progress, cancel, extensions: Mutex::new(HashMap::new()) }
    }
}

#[cfg(windows)]
fn is_cloud_placeholder(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
    const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x40000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x400000;
    meta.file_attributes() & (FILE_ATTRIBUTE_OFFLINE | FILE_ATTRIBUTE_RECALL_ON_OPEN | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS) != 0
}

#[cfg(not(windows))]
fn is_cloud_placeholder(_meta: &fs::Metadata) -> bool {
    false
}

pub fn extension_of(name: &str) -> Box<str> {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && ext.len() <= 12 => ext.to_ascii_lowercase().into(),
        _ => "".into(),
    }
}

/// Scan `path` recursively. Returns a partial tree if cancelled.
pub fn scan_dir(path: &Path, name: Box<str>, ctx: &ScanCtx) -> DirNode {
    let mut node = DirNode::named(name);
    if ctx.cancel.load(Relaxed) {
        return node;
    }

    let entries = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => {
            ctx.progress.errors.fetch_add(1, Relaxed);
            node.denied = true;
            return node;
        }
    };

    let mut subdirs = Vec::new();
    let mut local_ext: HashMap<Box<str>, (u64, u64)> = HashMap::new();
    let mut file_bytes = 0u64;

    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue; // symlinks + junctions: skip (Rust reports Windows mount points as symlinks)
        }
        if ft.is_dir() {
            subdirs.push(entry);
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            ctx.progress.errors.fetch_add(1, Relaxed);
            continue;
        };
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        node.files += 1;
        let modified = unix_secs(&meta);
        node.own_newest = node.own_newest.max(modified);
        node.markers |= marker_of(&file_name);

        if is_cloud_placeholder(&meta) {
            ctx.progress.cloud_bytes.fetch_add(meta.len(), Relaxed);
            continue;
        }
        let size = meta.len();
        file_bytes += size;
        let e = local_ext.entry(extension_of(&file_name)).or_default();
        e.0 += size;
        e.1 += 1;
        if size >= BIG_FILE {
            node.big_files.push(FileEntry { name: file_name.into(), size, modified });
        } else {
            node.small_count += 1;
            node.small_size += size;
        }
    }

    ctx.progress.files.fetch_add(node.files, Relaxed);
    ctx.progress.dirs.fetch_add(1, Relaxed);
    ctx.progress.bytes.fetch_add(file_bytes, Relaxed);
    if let Ok(mut cur) = ctx.progress.current.try_lock() {
        cur.clear();
        cur.push_str(&path.to_string_lossy());
    }
    if !local_ext.is_empty() {
        let mut global = ctx.extensions.lock().unwrap_or_else(|e| e.into_inner());
        for (k, (b, c)) in local_ext {
            let g = global.entry(k).or_default();
            g.0 += b;
            g.1 += c;
        }
    }

    node.children = subdirs
        .into_par_iter()
        .map(|e| {
            let name: Box<str> = e.file_name().to_string_lossy().into();
            scan_dir(&e.path(), name, ctx)
        })
        .collect();

    node.newest = node.children.iter().map(|c| c.newest).fold(node.own_newest, u64::max);
    node.size = file_bytes + node.children.iter().map(|c| c.size).sum::<u64>();
    node.files += node.children.iter().map(|c| c.files).sum::<u64>();
    node.dirs = node.children.len() as u64 + node.children.iter().map(|c| c.dirs).sum::<u64>();
    node.children.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    node.big_files.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    node
}

/// Remove a sub-directory or file from the tree (after it was moved to the Recycle Bin) and fix every ancestor's totals.
pub fn remove_from_tree(root: &mut DirNode, dir_path: &[u32], file_name: Option<&str>) -> Option<u64> {
    fn walk(node: &mut DirNode, rest: &[u32], file_name: Option<&str>) -> Option<(u64, u64, u64)> {
        let removed = match (rest, file_name) {
            ([], Some(f)) => {
                let i = node.big_files.iter().position(|x| &*x.name == f)?;
                let file = node.big_files.remove(i);
                (file.size, 1, 0)
            }
            ([], None) => return None,
            ([last], None) => {
                let child = node.children.remove(*last as usize);
                (child.size, child.files, child.dirs + 1)
            }
            ([first, tail @ ..], _) => walk(node.children.get_mut(*first as usize)?, tail, file_name)?,
        };
        node.size -= removed.0;
        node.files -= removed.1;
        node.dirs -= removed.2;
        node.children.sort_unstable_by(|a, b| b.size.cmp(&a.size));
        Some(removed)
    }
    walk(root, dir_path, file_name).map(|r| r.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, bytes: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&vec![7u8; bytes]).unwrap();
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        write(&r.join("video.mp4"), 3 * 1024 * 1024);
        write(&r.join("notes.txt"), 1000);
        write(&r.join("project/node_modules/lib/index.js"), 2000);
        write(&r.join("project/node_modules/big.bin"), 2 * 1024 * 1024);
        write(&r.join("project/src/main.rs"), 500);
        write(&r.join("empty/.keep"), 0);
        dir
    }

    #[test]
    fn totals_and_ordering() {
        let dir = fixture();
        let (p, c) = (Progress::default(), AtomicBool::new(false));
        let ctx = ScanCtx::new(&p, &c);
        let root = scan_dir(dir.path(), "root".into(), &ctx);

        let expected = 3 * 1024 * 1024 + 1000 + 2000 + 2 * 1024 * 1024 + 500;
        assert_eq!(root.size, expected as u64);
        assert_eq!(root.files, 6);
        assert_eq!(root.dirs, 5); // project, node_modules, lib, src, empty
        assert_eq!(&*root.children[0].name, "project");
        assert_eq!(&*root.big_files[0].name, "video.mp4");
        assert_eq!(root.small_count, 1);
        assert_eq!(p.files.load(Relaxed), 6);
        assert_eq!(p.bytes.load(Relaxed), expected as u64);

        let ext = ctx.extensions.lock().unwrap();
        assert_eq!(ext.get("mp4"), Some(&(3 * 1024 * 1024, 1)));
        assert_eq!(ext.get("keep"), None, "dot-files have no extension");
    }

    #[test]
    fn cancelled_scan_returns_quickly() {
        let dir = fixture();
        let (p, c) = (Progress::default(), AtomicBool::new(true));
        let root = scan_dir(dir.path(), "root".into(), &ScanCtx::new(&p, &c));
        assert_eq!(root.size, 0);
    }

    #[test]
    fn missing_dir_is_flagged_not_fatal() {
        let (p, c) = (Progress::default(), AtomicBool::new(false));
        let root = scan_dir(Path::new("Z:/definitely/not/here"), "x".into(), &ScanCtx::new(&p, &c));
        assert!(root.denied);
        assert_eq!(p.errors.load(Relaxed), 1);
    }

    #[test]
    fn remove_updates_ancestors() {
        let dir = fixture();
        let (p, c) = (Progress::default(), AtomicBool::new(false));
        let mut root = scan_dir(dir.path(), "root".into(), &ScanCtx::new(&p, &c));
        let before = root.size;
        // project is children[0]; node_modules is its largest child
        let nm_size = root.children[0].children[0].size;
        assert_eq!(remove_from_tree(&mut root, &[0, 0], None), Some(nm_size));
        assert_eq!(root.size, before - nm_size);
        assert_eq!(root.files, 4);

        let video = remove_from_tree(&mut root, &[], Some("video.mp4"));
        assert_eq!(video, Some(3 * 1024 * 1024));
        assert!(root.big_files.is_empty());
    }

    fn set_mtime(path: &Path, unix: u64) {
        let f = fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(unix)).unwrap();
    }

    #[test]
    fn records_mtime_newest_and_markers() {
        const OLD: u64 = 1_000_000_000; // 2001-09-09
        const MID: u64 = 1_500_000_000; // 2017-07-14
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        write(&r.join("app/package.json"), 20);
        write(&r.join("app/src/index.js"), 10);
        write(&r.join("app/big.bin"), 2 * 1024 * 1024);
        write(&r.join("rust/Cargo.toml"), 20);
        write(&r.join("rust/target/out.o"), 10);
        write(&r.join("dotnet/App.CSPROJ"), 20);
        write(&r.join("plain/readme.txt"), 5);
        set_mtime(&r.join("app/big.bin"), OLD);
        set_mtime(&r.join("app/src/index.js"), MID);
        set_mtime(&r.join("plain/readme.txt"), OLD);

        let (p, c) = (Progress::default(), AtomicBool::new(false));
        let root = scan_dir(r, "root".into(), &ScanCtx::new(&p, &c));
        let node = |n: &str| root.children.iter().find(|c| &*c.name == n).unwrap();

        // big file keeps its own mtime
        let big = &node("app").big_files[0];
        assert_eq!(&*big.name, "big.bin");
        assert_eq!(big.modified, OLD);
        // small files feed newest too: src/index.js is MID, package.json is "now"
        assert_eq!(node("app").children[0].newest, MID);
        assert_eq!(node("app").children[0].own_newest, MID);
        assert!(node("app").own_newest > MID, "package.json was just written");
        assert_eq!(node("app").newest, node("app").own_newest);
        assert_eq!(node("plain").newest, OLD);
        assert!(root.newest > MID && root.own_newest == 0, "root has no direct files");

        assert_eq!(node("app").markers, MARKER_JS);
        assert_eq!(node("rust").markers, MARKER_RUST);
        assert_eq!(node("dotnet").markers, MARKER_DOTNET, "extension match is case-insensitive");
        assert_eq!(node("plain").markers, 0);
        assert_eq!(node("app").children[0].markers, 0, "markers are per directory, not inherited");
    }

    #[test]
    fn marker_names() {
        assert_eq!(marker_of("package.json"), MARKER_JS);
        assert_eq!(marker_of("Package.JSON"), MARKER_JS);
        assert_eq!(marker_of("requirements.txt"), MARKER_PYTHON);
        assert_eq!(marker_of("build.gradle.kts"), MARKER_GRADLE);
        assert_eq!(marker_of("pom.xml"), MARKER_MAVEN);
        assert_eq!(marker_of("My.Solution.sln"), MARKER_DOTNET);
        assert_eq!(marker_of("lib.fsproj"), MARKER_DOTNET);
        assert_eq!(marker_of("pubspec.yaml"), MARKER_DART);
        assert_eq!(marker_of("go.mod"), MARKER_GO);
        assert_eq!(marker_of("Gemfile"), MARKER_RUBY);
        assert_eq!(marker_of("composer.json"), MARKER_PHP);
        assert_eq!(marker_of(".sln"), 0);
        assert_eq!(marker_of("package.json.bak"), 0);
        assert_eq!(marker_of("ไฟล์.txt"), 0);
    }

    /// Real-world benchmark (read-only): `DISKWHY_BENCH=C:\ cargo test --release bench_real -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_real() {
        let path = std::env::var("DISKWHY_BENCH").unwrap_or_else(|_| ".".into());
        let (p, c) = (Progress::default(), AtomicBool::new(false));
        let t = std::time::Instant::now();
        let root = scan_dir(Path::new(&path), path.clone().into(), &ScanCtx::new(&p, &c));
        let secs = t.elapsed().as_secs_f64();
        println!(
            "scanned {path}: {} files, {} dirs, {:.2} GB in {:.2}s ({:.0} files/s), {} unreadable, {:.2} GB cloud-only",
            root.files,
            root.dirs,
            root.size as f64 / 1e9,
            secs,
            root.files as f64 / secs,
            p.errors.load(Relaxed),
            p.cloud_bytes.load(Relaxed) as f64 / 1e9
        );
    }

    #[test]
    fn extensions() {
        assert_eq!(&*extension_of("Movie.MKV"), "mkv");
        assert_eq!(&*extension_of(".gitignore"), "");
        assert_eq!(&*extension_of("README"), "");
        assert_eq!(&*extension_of("archive.tar.gz"), "gz");
    }
}
