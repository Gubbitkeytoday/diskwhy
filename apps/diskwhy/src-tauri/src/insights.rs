//! Turns a scanned tree into answers: what kinds of files, which known space hogs, which giant files,
//! and which data looks unused (stale projects, forgotten big files, old installers, orphaned AppData).

use crate::installed;
use crate::scan::{self, DirNode};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/* ── File categories ─────────────────────────────────────────────────────── */

pub fn category_of(ext: &str) -> &'static str {
    match ext {
        "mp4" | "mkv" | "mov" | "avi" | "wmv" | "webm" | "m4v" | "flv" | "mts" | "m2ts" | "3gp" => "video",
        "jpg" | "jpeg" | "png" | "gif" | "heic" | "heif" | "webp" | "bmp" | "tif" | "tiff" | "raw" | "cr2" | "cr3" | "nef"
        | "arw" | "dng" | "psd" | "svg" => "image",
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "wma" | "aiff" => "audio",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "cab" => "archive",
        "iso" | "img" | "vhd" | "vhdx" | "vmdk" | "vdi" | "qcow2" | "wim" | "esd" => "disk-image",
        "exe" | "msi" | "msix" | "msixbundle" | "appx" | "appxbundle" => "installer",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "csv" | "odt" | "ods" | "md" | "rtf" | "epub" => {
            "document"
        }
        "dll" | "sys" | "drv" | "mui" | "cat" | "manifest" | "etl" | "evtx" => "system",
        "log" | "tmp" | "temp" | "dmp" | "bak" | "old" | "cache" => "temp-log",
        "js" | "ts" | "tsx" | "jsx" | "map" | "json" | "py" | "pyc" | "rs" | "rlib" | "go" | "java" | "class" | "jar" | "cs"
        | "cpp" | "c" | "h" | "o" | "obj" | "pdb" | "lib" | "a" | "node" | "wasm" => "code",
        "pak" | "upk" | "uasset" | "vpk" | "bsa" | "ba2" | "forge" | "bundle" | "assets" | "resource" => "game",
        "pst" | "ost" | "db" | "sqlite" | "mdb" | "accdb" | "ldf" | "mdf" => "database",
        _ => "other",
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct CategoryStat {
    pub category: &'static str,
    pub bytes: u64,
    pub count: u64,
}

#[derive(Serialize, Debug, Clone)]
pub struct ExtStat {
    pub ext: String,
    pub bytes: u64,
    pub count: u64,
}

pub fn categories(ext: &HashMap<Box<str>, (u64, u64)>) -> (Vec<CategoryStat>, Vec<ExtStat>) {
    let mut cats: HashMap<&'static str, (u64, u64)> = HashMap::new();
    for (e, (b, c)) in ext {
        let s = cats.entry(category_of(e)).or_default();
        s.0 += b;
        s.1 += c;
    }
    let mut cats: Vec<_> = cats.into_iter().map(|(category, (bytes, count))| CategoryStat { category, bytes, count }).collect();
    cats.sort_unstable_by(|a, b| b.bytes.cmp(&a.bytes));
    let mut exts: Vec<_> = ext
        .iter()
        .map(|(e, (b, c))| ExtStat { ext: if e.is_empty() { "(ไม่มีนามสกุล)".into() } else { e.to_string() }, bytes: *b, count: *c })
        .collect();
    exts.sort_unstable_by(|a, b| b.bytes.cmp(&a.bytes));
    exts.truncate(30);
    (cats, exts)
}

/* ── Known space hogs ────────────────────────────────────────────────────── */

/// How safe it is to reclaim the space.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Safety {
    /// Re-creatable / disposable: caches, temp, build output.
    Safe,
    /// Personal data or apps: look before you delete.
    Review,
    /// Managed by Windows: use the advised tool, never delete by hand.
    System,
}

struct Rule {
    id: &'static str,
    label: &'static str,
    advice: &'static str,
    safety: Safety,
    /// (lowercased full path with '\' separators, lowercased name) → match?
    test: fn(&str, &str) -> bool,
}

fn ends(path: &str, suffix: &str) -> bool {
    path.ends_with(suffix)
}

const RULES: &[Rule] = &[
    Rule {
        id: "recycle-bin",
        label: "ถังขยะ (Recycle Bin)",
        advice: "ไฟล์ที่ลบแล้วยังกินพื้นที่อยู่ — คลิกขวาที่ Recycle Bin บนเดสก์ท็อปแล้วเลือก Empty Recycle Bin",
        safety: Safety::Safe,
        test: |_, n| n == "$recycle.bin",
    },
    Rule {
        id: "temp",
        label: "ไฟล์ชั่วคราว (Temp)",
        advice: "ลบได้อย่างปลอดภัยผ่าน Settings › System › Storage › Temporary files หรือ Disk Cleanup",
        safety: Safety::Safe,
        test: |p, n| n == "temp" && (ends(p, "\\appdata\\local\\temp") || ends(p, "\\windows\\temp")),
    },
    Rule {
        id: "windows-update",
        label: "ไฟล์ดาวน์โหลดของ Windows Update",
        advice: "ล้างด้วย Disk Cleanup › Clean up system files › Windows Update Cleanup",
        safety: Safety::System,
        test: |p, _| ends(p, "\\windows\\softwaredistribution\\download"),
    },
    Rule {
        id: "windows-old",
        label: "Windows เวอร์ชันเก่า (Windows.old)",
        advice: "ถ้าไม่ต้องการย้อนกลับเวอร์ชัน ให้ลบผ่าน Disk Cleanup › Previous Windows installation(s)",
        safety: Safety::System,
        test: |p, n| n == "windows.old" && p.len() <= 15,
    },
    Rule {
        id: "winsxs",
        label: "คลังคอมโพเนนต์ Windows (WinSxS)",
        advice: "ห้ามลบเอง — รัน 'DISM /Online /Cleanup-Image /StartComponentCleanup' (ขนาดที่แสดงอาจสูงกว่าจริงเพราะ hard link)",
        safety: Safety::System,
        test: |p, _| ends(p, "\\windows\\winsxs"),
    },
    Rule {
        id: "node-modules",
        label: "node_modules (โปรเจกต์ JavaScript)",
        advice: "ติดตั้งใหม่ได้ด้วย npm/pnpm install — ลบของโปรเจกต์ที่ไม่ได้ใช้แล้วได้เลย",
        safety: Safety::Safe,
        test: |_, n| n == "node_modules",
    },
    Rule {
        id: "dev-caches",
        label: "แคชของเครื่องมือนักพัฒนา",
        advice: "ดาวน์โหลดใหม่ได้อัตโนมัติ — ใช้คำสั่งล้างของแต่ละตัว เช่น npm cache clean --force, pnpm store prune, pip cache purge",
        safety: Safety::Safe,
        test: |p, _| {
            ends(p, "\\appdata\\local\\npm-cache")
                || ends(p, "\\appdata\\local\\pnpm\\store")
                || ends(p, "\\appdata\\local\\yarn\\cache")
                || ends(p, "\\appdata\\local\\pip\\cache")
                || ends(p, "\\.gradle\\caches")
                || ends(p, "\\.m2\\repository")
                || ends(p, "\\.nuget\\packages")
                || ends(p, "\\.cargo\\registry")
                || ends(p, "\\appdata\\local\\bun\\install\\cache")
                || ends(p, "\\go\\pkg\\mod")
        },
    },
    Rule {
        id: "browser-cache",
        label: "แคชเบราว์เซอร์",
        advice: "ล้างจากเมนูของเบราว์เซอร์ (Ctrl+Shift+Delete › Cached images and files) ไม่กระทบรหัสผ่านหรือบุ๊กมาร์ก",
        safety: Safety::Safe,
        test: |p, n| {
            (n == "cache" || n == "code cache" || n == "cache_data")
                && (p.contains("\\google\\chrome\\user data\\")
                    || p.contains("\\microsoft\\edge\\user data\\")
                    || p.contains("\\bravesoftware\\")
                    || p.contains("\\mozilla\\firefox\\profiles\\"))
        },
    },
    Rule {
        id: "downloads",
        label: "โฟลเดอร์ดาวน์โหลด (Downloads)",
        advice: "มักมีไฟล์ติดตั้งและไฟล์ ZIP ที่ใช้แล้ว — เรียงตามขนาดแล้วลบสิ่งที่ไม่ต้องการ",
        safety: Safety::Review,
        test: |p, n| n == "downloads" && p.contains("\\users\\") && p.matches('\\').count() == 3,
    },
    Rule {
        id: "virtual-disks",
        label: "ดิสก์เสมือน (WSL / Docker / VM)",
        advice: "ไฟล์ .vhdx ไม่หดเองเมื่อลบข้อมูลข้างใน — รัน 'wsl --shutdown' แล้วใช้ Optimize-VHD หรือ diskpart compact vdisk",
        safety: Safety::Review,
        // Matched per file (.vhdx/.vhd/.vmdk) in `collect_vhdx`, never per folder, to avoid double counting.
        test: |_, _| false,
    },
    Rule {
        id: "games",
        label: "เกม (Steam / Epic / Xbox)",
        advice: "ถอนการติดตั้งเกมที่ไม่ได้เล่นจากตัว launcher เพื่อให้ลบครบทุกไฟล์",
        safety: Safety::Review,
        test: |p, _| ends(p, "\\steamapps\\common") || ends(p, "\\epic games") || ends(p, "\\xboxgames"),
    },
    Rule {
        id: "crash-dumps",
        label: "ไฟล์ crash dump",
        advice: "ใช้แค่ตอนวิเคราะห์ปัญหา — ลบได้ผ่าน Disk Cleanup › System error memory dump files",
        safety: Safety::Safe,
        test: |p, _| ends(p, "\\appdata\\local\\crashdumps") || ends(p, "\\windows\\minidump"),
    },
];

/// Special root-level system files.
const SYSTEM_FILES: &[(&str, &str, &str)] = &[
    ("hiberfil.sys", "ไฟล์ไฮเบอร์เนต (hiberfil.sys)", "ถ้าไม่ใช้โหมด Hibernate รัน 'powercfg /h off' ในฐานะผู้ดูแลระบบ (ห้ามลบเอง)"),
    ("pagefile.sys", "หน่วยความจำเสมือน (pagefile.sys)", "Windows จัดการให้อัตโนมัติ ปรับได้ที่ System › Advanced › Performance › Virtual memory"),
    ("swapfile.sys", "ไฟล์ swap (swapfile.sys)", "Windows จัดการให้อัตโนมัติ — ไม่ควรลบ"),
];

#[derive(Serialize, Debug, Clone)]
pub struct Location {
    pub path: String,
    pub size: u64,
    /// Index path into the tree (for "show in treemap").
    pub ids: Vec<u32>,
    /// Newest modification time (unix seconds) found here: the newest file inside a folder, or the file's own
    /// mtime. `None` when unknown. For stale projects it is the last time the *source* changed.
    pub last_used: Option<u64>,
    /// Short Thai explanation specific to this location, e.g. "ไม่ได้แก้ไขมา 14 เดือน".
    pub note: Option<String>,
}

impl Location {
    fn new(path: String, size: u64, ids: Vec<u32>, last_used: u64) -> Self {
        Self { path, size, ids, last_used: (last_used > 0).then_some(last_used), note: None }
    }

    fn with_note(mut self, note: String) -> Self {
        self.note = Some(note);
        self
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct Hog {
    pub id: &'static str,
    pub label: &'static str,
    pub advice: &'static str,
    pub safety: Safety,
    pub size: u64,
    pub count: u64,
    pub locations: Vec<Location>,
}

impl Hog {
    fn empty(id: &'static str, label: &'static str, advice: &'static str, safety: Safety) -> Self {
        Self { id, label, advice, safety, size: 0, count: 0, locations: vec![] }
    }

    fn add(&mut self, loc: Location) {
        self.size += loc.size;
        self.count += 1;
        self.locations.push(loc);
    }
}

/* ── Unused / stale data detectors ───────────────────────────────────────── */

const DAY: u64 = 86_400;
const MB: u64 = 1024 * 1024;

/// Big forgotten files: at least this large and untouched this long.
const STALE_FILE_MIN_SIZE: u64 = 100 * MB;
const STALE_FILE_AGE_DAYS: u64 = 365;
/// Developer projects whose source has not changed for this long have their rebuildable folders reported.
const STALE_PROJECT_AGE_DAYS: u64 = 90;
/// Installers / disk images / archives in Downloads.
const INSTALLER_MIN_SIZE: u64 = 10 * MB;
const INSTALLER_AGE_DAYS: u64 = 30;
/// AppData / ProgramData folders of programs that are no longer installed.
const ORPHAN_MIN_SIZE: u64 = 50 * MB;
const ORPHAN_AGE_DAYS: u64 = 90;

struct Detector {
    id: &'static str,
    label: &'static str,
    advice: &'static str,
    safety: Safety,
}

impl Detector {
    fn hog(&self) -> Hog {
        Hog::empty(self.id, self.label, self.advice, self.safety)
    }
}

const STALE_FILES: Detector = Detector {
    id: "stale-large-files",
    label: "ไฟล์ใหญ่ที่ไม่ได้แตะเกิน 1 ปี",
    advice: "ไฟล์ขนาด 100 MB ขึ้นไปที่ไม่ได้ถูกแก้ไขเลยเกิน 1 ปี (ดูจากเวลาแก้ไขล่าสุด) — ตรวจดูว่ายังต้องใช้อยู่ไหม แล้วย้ายไปถังขยะ (กู้คืนได้) หรือสำรองไว้ในไดรฟ์ภายนอก",
    safety: Safety::Review,
};

const STALE_PROJECTS: Detector = Detector {
    id: "stale-projects",
    label: "โปรเจกต์นักพัฒนาที่ไม่ได้แตะนาน (โฟลเดอร์ที่สร้างใหม่ได้)",
    advice: "โค้ดของโปรเจกต์เหล่านี้ไม่ได้ถูกแก้ไขเกิน 90 วันแล้ว — โฟลเดอร์ dependencies/build (node_modules, target, .venv, bin/obj ฯลฯ) สร้างกลับมาใหม่ได้ด้วย npm install / cargo build / pip install และโค้ดต้นฉบับของคุณจะไม่ถูกแตะ",
    safety: Safety::Safe,
};

const OLD_INSTALLERS: Detector = Detector {
    id: "old-installers",
    label: "ไฟล์ติดตั้งและไฟล์บีบอัดเก่าในโฟลเดอร์ดาวน์โหลด",
    advice: "ตัวติดตั้ง (.exe/.msi), ไฟล์ ISO และไฟล์ ZIP ขนาดใหญ่ที่ดาวน์โหลดไว้เกิน 30 วัน — ถ้าติดตั้งหรือแตกไฟล์เสร็จแล้วก็ลบได้ และดาวน์โหลดใหม่จากต้นทางได้เสมอ",
    safety: Safety::Review,
};

const ORPHANED_APPDATA: Detector = Detector {
    id: "orphaned-appdata",
    label: "ข้อมูลของโปรแกรมที่อาจถอนการติดตั้งแล้ว",
    advice: "ไม่พบชื่อโฟลเดอร์นี้ในรายการโปรแกรมที่ติดตั้งและไม่ได้ใช้งานมานาน — ตรวจดูชื่อให้แน่ใจว่าไม่ใช่โปรแกรมที่ยังใช้อยู่ก่อน แล้วย้ายไปถังขยะ (กู้คืนได้)",
    safety: Safety::Review,
};

/// Rebuildable output folders per ecosystem (matched case-insensitively against the *direct* children of a project).
const ARTIFACTS: &[(u16, &[&str])] = &[
    (scan::MARKER_JS, &["node_modules", ".next", ".nuxt", ".turbo", ".parcel-cache", ".svelte-kit", ".angular"]),
    (scan::MARKER_RUST, &["target"]),
    (scan::MARKER_PYTHON, &[".venv", "venv", ".tox", ".mypy_cache", ".pytest_cache"]),
    (scan::MARKER_GRADLE, &["build", ".gradle"]),
    (scan::MARKER_DOTNET, &["bin", "obj"]),
    (scan::MARKER_DART, &[".dart_tool", "build"]),
    (scan::MARKER_MAVEN, &["target"]),
];

/// Top-level folders where "old" files are normal or owned by Windows: never reported as stale files.
const STALE_EXCLUDED_DIRS: &[&str] = &["windows", "program files", "program files (x86)", "$recycle.bin", "system volume information"];

fn age_days(now: u64, t: u64) -> u64 {
    now.saturating_sub(t) / DAY
}

/// Whole months (30 days) for display, never below 1.
fn months(days: u64) -> u64 {
    (days / 30).max(1)
}

fn is_artifact(markers: u16, name: &str) -> bool {
    ARTIFACTS.iter().any(|(m, names)| markers & m != 0 && names.iter().any(|n| name.eq_ignore_ascii_case(n)))
}

fn ends_with_ci(name: &str, suffix: &str) -> bool {
    let (n, s) = (name.as_bytes(), suffix.as_bytes());
    n.len() >= s.len() && n[n.len() - s.len()..].eq_ignore_ascii_case(s)
}

fn is_virtual_disk(name: &str) -> bool {
    [".vhdx", ".vhd", ".vmdk"].iter().any(|e| ends_with_ci(name, e))
}

/// Path key for comparisons: lowercase with `\` separators.
fn key(path: &str) -> String {
    path.to_lowercase().replace('/', "\\")
}

/// `a` lies strictly inside directory `b` (both already `key`ed).
fn is_inside(a: &str, b: &str) -> bool {
    a.len() > b.len() && a.starts_with(b) && a.as_bytes()[b.len()] == b'\\'
}

/// Allocation-light path/index cursor for the O(nodes) passes below.
struct Cursor {
    path: String,
    ids: Vec<u32>,
}

impl Cursor {
    fn new(path: String, ids: Vec<u32>) -> Self {
        Self { path, ids }
    }

    /// Step into child `index`; returns a mark for [`Cursor::leave`].
    fn enter(&mut self, index: usize, name: &str) -> usize {
        let mark = self.path.len();
        self.path.push('\\');
        self.path.push_str(name);
        self.ids.push(index as u32);
        mark
    }

    fn leave(&mut self, mark: usize) {
        self.path.truncate(mark);
        self.ids.pop();
    }

    fn file_path(&self, name: &str) -> String {
        format!("{}\\{name}", self.path)
    }
}

/// A directory together with its path and index path.
struct Located<'a> {
    node: &'a DirNode,
    ids: Vec<u32>,
    path: String,
}

impl<'a> Located<'a> {
    fn child(&self, name: &str) -> Option<Located<'a>> {
        self.children().find(|c| c.node.name.eq_ignore_ascii_case(name))
    }

    fn children(&self) -> impl Iterator<Item = Located<'a>> + '_ {
        self.node.children.iter().enumerate().map(|(i, c)| {
            let mut ids = self.ids.clone();
            ids.push(i as u32);
            Located { node: c, ids, path: format!("{}\\{}", self.path, c.name) }
        })
    }
}

/// User profile folders: `<root>\Users\*`, or the children of `root` if it already is `Users`,
/// or `root` itself if it is a profile (`...\Users\<name>`).
fn user_profiles<'a>(root: &'a DirNode, base: &str) -> Vec<Located<'a>> {
    let segs: Vec<&str> = base.split(['\\', '/']).filter(|s| !s.is_empty()).collect();
    let top = || Located { node: root, ids: vec![], path: base.to_string() };
    match segs.as_slice() {
        [.., users, _] if users.eq_ignore_ascii_case("users") => vec![top()],
        [.., users] if users.eq_ignore_ascii_case("users") => top().children().collect(),
        _ => top().child("Users").map(|u| u.children().collect()).unwrap_or_default(),
    }
}

/// Folders never entered when looking for projects: other tools' dependency trees, Windows-managed
/// areas, installed Electron apps (`resources\app` ships a package.json + node_modules that must stay) and every
/// dot-folder (`.vscode\extensions`, `.kiro`, `.cargo`, `.platformio`, … are installed tools, not the user's projects:
/// removing their node_modules would break them).
fn skip_for_projects(parent_name: &str, name: &str, at_root: bool) -> bool {
    const ANY_DEPTH: &[&str] = &["node_modules", "site-packages", "appdata", "$recycle.bin", "system volume information", "windowsapps", "program files", "program files (x86)"];
    const ROOT_ONLY: &[&str] = &["windows", "programdata"];
    name.starts_with('.')
        || ANY_DEPTH.iter().any(|n| name.eq_ignore_ascii_case(n))
        || (at_root && ROOT_ONLY.iter().any(|n| name.eq_ignore_ascii_case(n)))
        || (parent_name.eq_ignore_ascii_case("resources") && (name.eq_ignore_ascii_case("app") || name.eq_ignore_ascii_case("app.asar.unpacked")))
}

/// Last path segment, i.e. the project folder name.
fn last_segment(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

struct ProjectScan {
    hog: Hog,
    /// Index paths of the artifact folders reported here (the rule pass must skip them).
    claimed: HashSet<Vec<u32>>,
    now: u64,
}

/// Find developer projects whose source has been untouched for `STALE_PROJECT_AGE_DAYS` and report their
/// rebuildable folders. Staleness = newest of the project's own files and of its sub-folders that are neither
/// build artifacts nor `.git` (so `npm install` or `git fetch` don't make a dead project look alive).
fn find_stale_projects(root: &DirNode, base: &str, now: u64) -> (Hog, HashSet<Vec<u32>>) {
    let mut scan = ProjectScan { hog: STALE_PROJECTS.hog(), claimed: HashSet::new(), now };
    visit_projects(root, &mut Cursor::new(base.to_string(), vec![]), &mut scan);
    (scan.hog, scan.claimed)
}

fn visit_projects(node: &DirNode, cur: &mut Cursor, scan: &mut ProjectScan) {
    let markers = node.markers;
    if markers != 0 {
        let source_newest = node
            .children
            .iter()
            .filter(|c| !is_artifact(markers, &c.name) && !c.name.eq_ignore_ascii_case(".git"))
            .map(|c| c.newest)
            .fold(node.own_newest, u64::max);
        let idle = age_days(scan.now, source_newest);
        if source_newest > 0 && idle >= STALE_PROJECT_AGE_DAYS {
            let note = format!("โปรเจกต์ {} ไม่ได้แก้ไขมา {} เดือน", last_segment(&cur.path), months(idle));
            for (i, c) in node.children.iter().enumerate() {
                if c.size > 0 && is_artifact(markers, &c.name) {
                    let mark = cur.enter(i, &c.name);
                    scan.claimed.insert(cur.ids.clone());
                    scan.hog.add(Location::new(cur.path.clone(), c.size, cur.ids.clone(), source_newest).with_note(note.clone()));
                    cur.leave(mark);
                }
            }
        }
    }
    let at_root = cur.ids.is_empty();
    for (i, c) in node.children.iter().enumerate() {
        // Artifacts are never descended into (reported or not): node_modules is full of nested package.json files.
        if (markers != 0 && is_artifact(markers, &c.name)) || skip_for_projects(&node.name, &c.name, at_root) {
            continue;
        }
        let mark = cur.enter(i, &c.name);
        visit_projects(c, cur, scan);
        cur.leave(mark);
    }
}

struct FileScan<'a> {
    hits: &'a mut HashMap<&'static str, Hog>,
    claimed: &'a HashSet<Vec<u32>>,
    stale: Hog,
    now: u64,
}

fn is_system_file(name: &str) -> bool {
    SYSTEM_FILES.iter().any(|(n, _, _)| name.eq_ignore_ascii_case(n))
}

/// One pass over every big file: virtual disks (anywhere) and forgotten files (outside Windows-owned areas,
/// system files, virtual disks and folders already reported by `stale-projects`).
fn scan_files(node: &DirNode, cur: &mut Cursor, skip_stale: bool, fs: &mut FileScan) {
    for f in &node.big_files {
        if is_virtual_disk(&f.name) {
            let hog = fs.hits.entry("virtual-disks").or_insert_with(|| {
                let r = RULES.iter().find(|r| r.id == "virtual-disks").expect("rule exists");
                Hog::empty(r.id, r.label, r.advice, r.safety)
            });
            hog.add(Location::new(cur.file_path(&f.name), f.size, cur.ids.clone(), f.modified));
        } else if !skip_stale
            && f.size >= STALE_FILE_MIN_SIZE
            && f.modified > 0
            && age_days(fs.now, f.modified) >= STALE_FILE_AGE_DAYS
            && !is_system_file(&f.name)
        {
            let note = format!("ไม่ได้แก้ไขมา {} เดือน", months(age_days(fs.now, f.modified)));
            fs.stale.add(Location::new(cur.file_path(&f.name), f.size, cur.ids.clone(), f.modified).with_note(note));
        }
    }
    for (i, c) in node.children.iter().enumerate() {
        let mark = cur.enter(i, &c.name);
        let excluded_top = cur.ids.len() == 1 && STALE_EXCLUDED_DIRS.iter().any(|d| c.name.eq_ignore_ascii_case(d));
        // Chromium-family profiles ("…\Chrome|Edge|Brave\User Data") hold cache blobs that look old and big;
        // they belong to the browser-cache finding, not to "forgotten files".
        let browser_profile = c.name.eq_ignore_ascii_case("User Data");
        let skip = skip_stale || excluded_top || browser_profile || (!fs.claimed.is_empty() && fs.claimed.contains(cur.ids.as_slice()));
        scan_files(c, cur, skip, fs);
        cur.leave(mark);
    }
}

/// Installers, disk images and archives lying around in a Downloads folder.
fn collect_old_installers(node: &DirNode, cur: &mut Cursor, now: u64, hog: &mut Hog) {
    for f in &node.big_files {
        if f.size < INSTALLER_MIN_SIZE || f.modified == 0 || is_virtual_disk(&f.name) {
            continue;
        }
        let age = age_days(now, f.modified);
        if age < INSTALLER_AGE_DAYS || !matches!(category_of(&scan::extension_of(&f.name)), "installer" | "disk-image" | "archive") {
            continue;
        }
        let note = format!("ดาวน์โหลดไว้ {} เดือนก่อน", months(age));
        hog.add(Location::new(cur.file_path(&f.name), f.size, cur.ids.clone(), f.modified).with_note(note));
    }
    for (i, c) in node.children.iter().enumerate() {
        let mark = cur.enter(i, &c.name);
        collect_old_installers(c, cur, now, hog);
        cur.leave(mark);
    }
}

/// Folders directly inside `<profile>\AppData\{Local,Roaming,LocalLow}` and `<root>\ProgramData` that are big,
/// unused for a long time, match no installed program and are not already reported by another rule.
///
/// `reported` holds the `key`ed paths of every location the other rules found.
fn find_orphaned_appdata(root: &DirNode, base: &str, installed: &[String], now: u64, reported: &[String]) -> Hog {
    let mut hog = ORPHANED_APPDATA.hog();
    let top = Located { node: root, ids: vec![], path: base.to_string() };
    let mut parents: Vec<Located> = Vec::new();
    for profile in user_profiles(root, base) {
        if let Some(appdata) = profile.child("AppData") {
            parents.extend(["Local", "Roaming", "LocalLow"].iter().filter_map(|n| appdata.child(n)));
        }
    }
    parents.extend(top.child("ProgramData"));

    for parent in &parents {
        for cand in parent.children() {
            let (size, newest) = (cand.node.size, cand.node.newest);
            if size < ORPHAN_MIN_SIZE || newest == 0 || age_days(now, newest) < ORPHAN_AGE_DAYS {
                continue;
            }
            if installed::folder_is_known(&cand.node.name, installed) {
                continue;
            }
            let k = key(&cand.path);
            if reported.iter().any(|r| *r == k || is_inside(r, &k) || is_inside(&k, r)) {
                continue;
            }
            let note = format!("ไม่ได้ใช้งานมา {} เดือน", months(age_days(now, newest)));
            hog.add(Location::new(cand.path, size, cand.ids, newest).with_note(note));
        }
    }
    hog
}

/// Locations kept per hog (the totals `size`/`count` always cover everything found).
fn max_locations(id: &str) -> usize {
    match id {
        "stale-large-files" => 50,
        "stale-projects" => 40,
        _ => 20,
    }
}

/// Detect space hogs and unused data in a finished scan, using the real installed-program list and clock.
pub fn find_hogs(root: &DirNode, root_path: &Path) -> Vec<Hog> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    find_hogs_with(root, root_path, installed::installed_program_tokens(), now)
}

/// [`find_hogs`] with the installed-program tokens and "now" (unix seconds) supplied by the caller.
pub fn find_hogs_with(root: &DirNode, root_path: &Path, installed: &[String], now: u64) -> Vec<Hog> {
    let mut hits: HashMap<&'static str, Hog> = HashMap::new();
    let base = root_path.to_string_lossy().trim_end_matches(['\\', '/']).to_string();

    // 1. Stale projects first: the folders they claim must not be counted again by the rules below.
    let (stale_projects, claimed) = find_stale_projects(root, &base, now);

    // 2. Known space hogs by path/name.
    fn visit(node: &DirNode, path: &str, ids: &mut Vec<u32>, claimed: &HashSet<Vec<u32>>, hits: &mut HashMap<&'static str, Hog>) {
        if !claimed.is_empty() && claimed.contains(ids.as_slice()) {
            return;
        }
        let lower = path.to_lowercase().replace('/', "\\");
        let name = node.name.to_lowercase();
        if let Some(rule) = RULES.iter().find(|r| (r.test)(&lower, &name)) {
            hits.entry(rule.id)
                .or_insert_with(|| Hog::empty(rule.id, rule.label, rule.advice, rule.safety))
                .add(Location::new(path.to_string(), node.size, ids.clone(), node.newest));
            return; // don't double count nested matches (e.g. node_modules inside node_modules)
        }
        for (i, child) in node.children.iter().enumerate() {
            ids.push(i as u32);
            visit(child, &format!("{path}\\{}", child.name), ids, claimed, hits);
            ids.pop();
        }
    }

    let mut ids = Vec::new();
    for (i, child) in root.children.iter().enumerate() {
        ids.push(i as u32);
        visit(child, &format!("{base}\\{}", child.name), &mut ids, &claimed, &mut hits);
        ids.pop();
    }

    // 3. Root-level system files.
    for f in &root.big_files {
        if let Some((_, label, advice)) = SYSTEM_FILES.iter().find(|(n, _, _)| f.name.eq_ignore_ascii_case(n)) {
            let mut hog = Hog::empty("system-file", label, advice, Safety::System);
            hog.add(Location::new(format!("{base}\\{}", f.name), f.size, vec![], f.modified));
            hits.insert(label, hog);
        }
    }

    // 4. Virtual disks anywhere + forgotten big files (one pass).
    let root_excluded = base.split(['\\', '/']).any(|seg| STALE_EXCLUDED_DIRS.iter().any(|d| seg.eq_ignore_ascii_case(d)));
    let mut files = FileScan { hits: &mut hits, claimed: &claimed, stale: STALE_FILES.hog(), now };
    scan_files(root, &mut Cursor::new(base.clone(), vec![]), root_excluded, &mut files);
    let stale_files = files.stale;

    // 5. Orphaned AppData (needs the paths every rule above already reported).
    let reported: Vec<String> = hits.values().chain([&stale_projects]).flat_map(|h| h.locations.iter().map(|l| key(&l.path))).collect();
    let orphaned = find_orphaned_appdata(root, &base, installed, now, &reported);
    // A forgotten file inside an orphaned folder is already counted there — never report the bytes twice.
    let mut stale_files = stale_files;
    if !orphaned.locations.is_empty() {
        let roots: Vec<String> = orphaned.locations.iter().map(|l| format!("{}\\", key(&l.path))).collect();
        stale_files.locations.retain(|l| {
            let p = key(&l.path);
            !roots.iter().any(|r| p.starts_with(r.as_str()))
        });
        stale_files.size = stale_files.locations.iter().map(|l| l.size).sum();
        stale_files.count = stale_files.locations.len() as u64;
    }

    // 6. Old installers in every user's Downloads.
    let mut old_installers = OLD_INSTALLERS.hog();
    for dl in user_profiles(root, &base).iter().filter_map(|p| p.child("Downloads")) {
        collect_old_installers(dl.node, &mut Cursor::new(dl.path.clone(), dl.ids.clone()), now, &mut old_installers);
    }

    for hog in [stale_projects, stale_files, old_installers, orphaned] {
        hits.insert(hog.id, hog);
    }

    let mut out: Vec<Hog> = hits.into_values().filter(|h| h.size > 0).collect();
    for h in &mut out {
        h.locations.sort_unstable_by(|a, b| b.size.cmp(&a.size));
        h.locations.truncate(max_locations(h.id));
    }
    out.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    out
}

/* ── Largest files ───────────────────────────────────────────────────────── */

#[derive(Serialize, Debug, Clone)]
pub struct TopFile {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub category: &'static str,
    /// Index path of the containing directory.
    pub dir_ids: Vec<u32>,
    /// Last-write time, unix seconds (0 if unknown).
    pub modified: u64,
}

pub fn top_files(root: &DirNode, root_path: &Path, limit: usize) -> Vec<TopFile> {
    struct Raw {
        size: u64,
        modified: u64,
        path: String,
        name: String,
        dir_ids: Vec<u32>,
    }
    let mut all: Vec<Raw> = Vec::new();
    fn visit(node: &DirNode, cur: &mut Cursor, out: &mut Vec<Raw>) {
        for f in &node.big_files {
            out.push(Raw { size: f.size, modified: f.modified, path: cur.file_path(&f.name), name: f.name.to_string(), dir_ids: cur.ids.clone() });
        }
        for (i, c) in node.children.iter().enumerate() {
            let mark = cur.enter(i, &c.name);
            visit(c, cur, out);
            cur.leave(mark);
        }
    }
    let base = root_path.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
    visit(root, &mut Cursor::new(base, vec![]), &mut all);
    all.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    all.truncate(limit);
    all.into_iter()
        .map(|r| {
            let category = category_of(&scan::extension_of(&r.name));
            TopFile { path: r.path, name: r.name, size: r.size, category, dir_ids: r.dir_ids, modified: r.modified }
        })
        .collect()
}

/* ── Node view (treemap + list) ──────────────────────────────────────────── */

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChildView {
    pub kind: &'static str, // "dir" | "file" | "small" | "rest"
    pub name: String,
    pub size: u64,
    pub files: u64,
    /// For dirs: index among children (to drill down).
    pub index: Option<u32>,
    pub category: Option<&'static str>,
    pub denied: bool,
    /// Dirs: newest file inside (unix seconds); files: the file's mtime; "small"/"rest" tiles: 0.
    pub newest: u64,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeView {
    pub ids: Vec<u32>,
    pub path: String,
    pub name: String,
    pub size: u64,
    pub files: u64,
    pub dirs: u64,
    pub crumbs: Vec<(String, Vec<u32>)>,
    pub children: Vec<ChildView>,
}

pub fn node_view(root: &DirNode, root_path: &Path, ids: &[u32], limit: usize) -> Option<NodeView> {
    let base = root_path.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
    let mut node = root;
    let mut path = base.clone();
    let mut crumbs = vec![(root_path.to_string_lossy().to_string(), vec![])];
    for (depth, &i) in ids.iter().enumerate() {
        node = node.children.get(i as usize)?;
        path = format!("{path}\\{}", node.name);
        crumbs.push((node.name.to_string(), ids[..=depth].to_vec()));
    }

    let mut children: Vec<ChildView> = node
        .children
        .iter()
        .enumerate()
        .map(|(i, c)| ChildView {
            kind: "dir",
            name: c.name.to_string(),
            size: c.size,
            files: c.files,
            index: Some(i as u32),
            category: None,
            denied: c.denied,
            newest: c.newest,
        })
        .chain(node.big_files.iter().map(|f| ChildView {
            kind: "file",
            name: f.name.to_string(),
            size: f.size,
            files: 1,
            index: None,
            category: Some(category_of(&scan::extension_of(&f.name))),
            denied: false,
            newest: f.modified,
        }))
        .collect();
    if node.small_count > 0 {
        children.push(ChildView {
            kind: "small",
            name: format!("ไฟล์ขนาดเล็ก {} ไฟล์", node.small_count),
            size: node.small_size,
            files: node.small_count,
            index: None,
            category: None,
            denied: false,
            newest: 0,
        });
    }
    children.sort_by(|a, b| b.size.cmp(&a.size));
    if children.len() > limit {
        let rest: Vec<_> = children.split_off(limit);
        children.push(ChildView {
            kind: "rest",
            name: format!("อื่น ๆ อีก {} รายการ", rest.len()),
            size: rest.iter().map(|c| c.size).sum(),
            files: rest.iter().map(|c| c.files).sum(),
            index: None,
            category: None,
            denied: false,
            newest: 0,
        });
    }

    Some(NodeView {
        ids: ids.to_vec(),
        path,
        name: node.name.to_string(),
        size: node.size,
        files: node.files,
        dirs: node.dirs,
        crumbs,
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{DirNode, FileEntry, MARKER_DART, MARKER_DOTNET, MARKER_JS, MARKER_PYTHON, MARKER_RUST};

    /// Fixed clock for every time-based test (never the real one).
    const NOW: u64 = 1_800_000_000;

    fn ago(days: u64) -> u64 {
        NOW - days * DAY
    }

    fn dir(name: &str, size: u64, children: Vec<DirNode>) -> DirNode {
        DirNode { name: name.into(), size, children, ..Default::default() }
    }

    /// Directory with `own_size` bytes of small files last written at `own_newest`; size/newest roll up from the children.
    fn tree(name: &str, own_size: u64, own_newest: u64, markers: u16, children: Vec<DirNode>) -> DirNode {
        let size = own_size + children.iter().map(|c| c.size).sum::<u64>();
        let newest = children.iter().map(|c| c.newest).fold(own_newest, u64::max);
        DirNode { name: name.into(), size, newest, own_newest, markers, children, ..Default::default() }
    }

    fn leaf(name: &str, size: u64, newest: u64) -> DirNode {
        tree(name, size, newest, 0, vec![])
    }

    /// Plain container directory (no files of its own).
    fn parent(name: &str, children: Vec<DirNode>) -> DirNode {
        tree(name, 0, 0, 0, children)
    }

    fn fe(name: &str, size: u64, modified: u64) -> FileEntry {
        FileEntry { name: name.into(), size, modified }
    }

    /// Attach big files to a directory (updates its size and newest).
    fn with_files(mut d: DirNode, files: Vec<FileEntry>) -> DirNode {
        for f in &files {
            d.size += f.size;
            d.own_newest = d.own_newest.max(f.modified);
            d.newest = d.newest.max(f.modified);
        }
        d.big_files = files;
        d
    }

    fn drive(children: Vec<DirNode>) -> DirNode {
        parent("C:\\", children)
    }

    fn run(root: &DirNode, installed: &[&str]) -> Vec<Hog> {
        let tokens: Vec<String> = installed.iter().map(|s| s.to_string()).collect();
        find_hogs_with(root, Path::new("C:\\"), &tokens, NOW)
    }

    fn get<'a>(hogs: &'a [Hog], id: &str) -> Option<&'a Hog> {
        hogs.iter().find(|h| h.id == id)
    }

    fn paths(h: &Hog) -> Vec<&str> {
        h.locations.iter().map(|l| l.path.as_str()).collect()
    }

    fn sorted_paths(h: &Hog) -> Vec<String> {
        let mut p: Vec<String> = h.locations.iter().map(|l| l.path.clone()).collect();
        p.sort();
        p
    }

    fn sorted(mut v: Vec<String>) -> Vec<String> {
        v.sort();
        v
    }

    fn sample() -> DirNode {
        let mut root = dir(
            "C:\\",
            1000,
            vec![
                dir(
                    "Users",
                    600,
                    vec![dir(
                        "somchai",
                        600,
                        vec![
                            dir("Downloads", 300, vec![]),
                            dir("AppData", 200, vec![dir("Local", 200, vec![dir("Temp", 150, vec![]), dir("npm-cache", 50, vec![])])]),
                            dir("code", 100, vec![dir("app", 100, vec![dir("node_modules", 90, vec![dir("node_modules", 10, vec![])])])]),
                        ],
                    )],
                ),
                dir("$Recycle.Bin", 50, vec![]),
            ],
        );
        root.big_files = vec![fe("hiberfil.sys", 350, 0)];
        root
    }

    #[test]
    fn detects_hogs_without_double_counting() {
        let hogs = run(&sample(), &[]);
        let sz = |id: &str| get(&hogs, id).map(|h| (h.size, h.count));
        assert_eq!(sz("downloads"), Some((300, 1)));
        assert_eq!(sz("temp"), Some((150, 1)));
        assert_eq!(sz("dev-caches"), Some((50, 1)));
        assert_eq!(sz("node-modules"), Some((90, 1)), "nested node_modules must not be counted twice");
        assert_eq!(sz("recycle-bin"), Some((50, 1)));
        assert_eq!(sz("system-file"), Some((350, 1)));
        let temp = get(&hogs, "temp").unwrap();
        assert_eq!(temp.locations[0].path, "C:\\Users\\somchai\\AppData\\Local\\Temp");
        assert_eq!(temp.locations[0].ids, vec![0, 0, 1, 0, 0]);
        assert_eq!(temp.locations[0].last_used, None, "unknown time stays None");
        assert_eq!(temp.locations[0].note, None);
        assert_eq!(hogs[0].size, 350, "sorted largest first");
        assert!(hogs.iter().all(|h| h.size > 0), "empty detectors are not emitted");
    }

    #[test]
    fn existing_rules_report_last_used() {
        let mut root = drive(vec![parent("Users", vec![parent("a", vec![leaf("Downloads", 300, ago(7))])]), leaf("$Recycle.Bin", 50, 0)]);
        root.big_files = vec![fe("hiberfil.sys", 350, ago(3))];
        let hogs = run(&root, &[]);
        assert_eq!(get(&hogs, "downloads").unwrap().locations[0].last_used, Some(ago(7)));
        assert_eq!(get(&hogs, "recycle-bin").unwrap().locations[0].last_used, None);
        assert_eq!(get(&hogs, "system-file").unwrap().locations[0].last_used, Some(ago(3)));
    }

    #[test]
    fn wrapper_uses_real_clock_and_registry_without_panicking() {
        let hogs = find_hogs(&sample(), Path::new("C:\\"));
        assert!(get(&hogs, "node-modules").is_some());
    }

    /* ── stale projects ── */

    fn projects_fixture() -> DirNode {
        let code = parent(
            "code",
            vec![
                // JS: source untouched 150 days; node_modules/.next/.git were touched yesterday (must not count as activity)
                tree(
                    "old-web",
                    10,
                    ago(200),
                    MARKER_JS,
                    vec![leaf("src", 50, ago(150)), leaf("node_modules", 900, ago(1)), leaf(".git", 5, ago(1)), leaf(".next", 100, ago(1))],
                ),
                // JS: a source edit 10 days ago keeps it alive
                tree("fresh-web", 10, ago(200), MARKER_JS, vec![leaf("src", 50, ago(10)), leaf("node_modules", 400, ago(1))]),
                tree("old-rust", 5, ago(100), MARKER_RUST, vec![leaf("src", 20, ago(100)), leaf("target", 2000, ago(2))]),
                tree("old-api", 5, ago(95), MARKER_PYTHON, vec![leaf(".venv", 300, ago(95)), leaf("app", 20, ago(96))]),
                tree("old-net", 5, ago(120), MARKER_DOTNET, vec![leaf("bin", 50, ago(120)), leaf("obj", 70, ago(120)), leaf("Program", 9, ago(121))]),
                tree("old-dart", 5, ago(130), MARKER_DART, vec![leaf(".dart_tool", 40, ago(130)), leaf("build", 60, ago(130)), leaf("bin", 7, ago(130))]),
                // exactly on / just inside the 90-day line
                tree("edge-90", 1, ago(90), MARKER_JS, vec![leaf("node_modules", 11, ago(1))]),
                tree("edge-89", 1, ago(89), MARKER_JS, vec![leaf("node_modules", 10, ago(1))]),
                // not a project (no manifest): handled by the plain node_modules rule only
                tree("plain", 5, ago(300), 0, vec![leaf("node_modules", 60, ago(300))]),
            ],
        );
        drive(vec![parent("Users", vec![parent("ann", vec![code])])])
    }

    #[test]
    fn stale_projects_report_only_rebuildable_folders_of_untouched_projects() {
        let hogs = run(&projects_fixture(), &[]);
        let h = get(&hogs, "stale-projects").expect("stale-projects");
        assert_eq!(h.safety, Safety::Safe);
        let b = "C:\\Users\\ann\\code";
        let want = sorted(vec![
            format!("{b}\\old-web\\node_modules"),
            format!("{b}\\old-web\\.next"),
            format!("{b}\\old-rust\\target"),
            format!("{b}\\old-api\\.venv"),
            format!("{b}\\old-net\\bin"),
            format!("{b}\\old-net\\obj"),
            format!("{b}\\old-dart\\.dart_tool"),
            format!("{b}\\old-dart\\build"),
            format!("{b}\\edge-90\\node_modules"),
        ]);
        assert_eq!(sorted_paths(h), want);
        assert_eq!((h.size, h.count), (900 + 100 + 2000 + 300 + 50 + 70 + 40 + 60 + 11, 9));
    }

    #[test]
    fn stale_project_locations_carry_age_note_ids_and_source_time() {
        let hogs = run(&projects_fixture(), &[]);
        let h = get(&hogs, "stale-projects").unwrap();
        let loc = |suffix: &str| h.locations.iter().find(|l| l.path.ends_with(suffix)).unwrap_or_else(|| panic!("{suffix}"));

        let nm = loc("old-web\\node_modules");
        assert_eq!(nm.note.as_deref(), Some("โปรเจกต์ old-web ไม่ได้แก้ไขมา 5 เดือน")); // 150 days
        assert_eq!(nm.last_used, Some(ago(150)), "last SOURCE change, not the recent npm install");
        // C:\ > Users > ann > code > old-web(0) > node_modules(1)
        assert_eq!(nm.ids, vec![0, 0, 0, 0, 1]);
        assert_eq!(loc("old-rust\\target").note.as_deref(), Some("โปรเจกต์ old-rust ไม่ได้แก้ไขมา 3 เดือน"));
        assert_eq!(loc("old-net\\bin").note.as_deref(), Some("โปรเจกต์ old-net ไม่ได้แก้ไขมา 4 เดือน"));
        assert_eq!(loc("edge-90\\node_modules").note.as_deref(), Some("โปรเจกต์ edge-90 ไม่ได้แก้ไขมา 3 เดือน"));
    }

    #[test]
    fn recent_source_edit_keeps_project_out() {
        let hogs = run(&projects_fixture(), &[]);
        let h = get(&hogs, "stale-projects").unwrap();
        assert!(!h.locations.iter().any(|l| l.path.contains("fresh-web") || l.path.contains("edge-89") || l.path.contains("plain")));
        // ecosystems don't borrow each other's folder names: `bin` is not an artifact of a Dart project
        assert!(!h.locations.iter().any(|l| l.path.ends_with("old-dart\\bin") || l.path.ends_with("Program")));
    }

    #[test]
    fn claimed_node_modules_are_not_counted_twice() {
        let hogs = run(&projects_fixture(), &[]);
        let nm = get(&hogs, "node-modules").unwrap();
        // fresh-web 400 + edge-89 10 + plain 60; old-web 900 and edge-90 11 belong to stale-projects
        assert_eq!((nm.size, nm.count), (470, 3));
        let sp = get(&hogs, "stale-projects").unwrap();
        for l in &nm.locations {
            assert!(!sp.locations.iter().any(|s| s.path == l.path), "{} reported twice", l.path);
        }
    }

    #[test]
    fn project_pass_ignores_nested_manifests_in_node_modules_and_electron_apps() {
        let pkg = tree("left-pad", 1, ago(500), MARKER_JS, vec![leaf("build", 10_000, ago(500))]);
        let app = tree("proj", 1, ago(400), MARKER_JS, vec![parent("node_modules", vec![pkg]), leaf("src", 1, ago(400))]);
        // an installed Electron app ships resources\app\package.json + node_modules and must never be offered
        let electron = parent(
            "SomeApp",
            vec![parent("resources", vec![tree("app", 1, ago(900), MARKER_JS, vec![leaf("node_modules", 5000, ago(900))])])],
        );
        let root = drive(vec![parent("dev", vec![app, electron])]);
        let h = get(&run(&root, &[]), "stale-projects").cloned().expect("proj is stale");
        assert_eq!(paths(&h), vec!["C:\\dev\\proj\\node_modules"], "no left-pad/build, no Electron app");
    }

    #[test]
    fn project_pass_skips_appdata_and_program_files() {
        let proj = |n: &str| tree(n, 1, ago(400), MARKER_JS, vec![leaf("node_modules", 700, ago(1))]);
        let root = drive(vec![
            parent("Program Files", vec![proj("Tool")]),
            parent(
                "Users",
                vec![parent(
                    "ann",
                    vec![
                        parent("AppData", vec![parent("Local", vec![proj("Programs")])]),
                        // installed editor extensions / tool homes live in dot-folders
                        parent(".kiro", vec![parent("extensions", vec![proj("some.extension-1.0")])]),
                        parent(".tools", vec![proj("mcp")]),
                    ],
                )],
            ),
            proj("Dev"),
        ]);
        let h = get(&run(&root, &[]), "stale-projects").cloned().unwrap();
        assert_eq!(paths(&h), vec!["C:\\Dev\\node_modules"]);
    }

    #[test]
    fn scanning_a_project_folder_directly_works() {
        let proj = tree("app", 1, ago(200), MARKER_RUST, vec![leaf("target", 400, ago(1))]);
        let h = find_hogs_with(&proj, Path::new("D:\\work\\app"), &[], NOW);
        let sp = get(&h, "stale-projects").unwrap();
        assert_eq!(paths(sp), vec!["D:\\work\\app\\target"]);
        assert_eq!(sp.locations[0].ids, vec![0]);
        assert_eq!(sp.locations[0].note.as_deref(), Some("โปรเจกต์ app ไม่ได้แก้ไขมา 6 เดือน"));
    }

    /* ── old installers ── */

    #[test]
    fn old_installers_respect_age_size_and_category_thresholds() {
        let sub = with_files(leaf("tools", 0, 0), vec![fe("tool.msi", 12 * MB, ago(100)), fe("notes.txt", 50 * MB, ago(100))]);
        let downloads = with_files(
            parent("Downloads", vec![sub]),
            vec![
                fe("setup.exe", 50 * MB, ago(60)),
                fe("old.zip", 20 * MB, ago(31)),
                fe("exact-age.exe", 10 * MB, ago(30)),
                fe("too-young.exe", 10 * MB, ago(29)),
                fe("just-small.exe", 10 * MB - 1, ago(400)),
                fe("recent.iso", 2000 * MB, ago(5)),
                fe("ubuntu.ISO", 3000 * MB, ago(45)),
                fe("report.pdf", 100 * MB, ago(400)),
                fe("unknown-time.exe", 80 * MB, 0),
                fe("vm.vhdx", 900 * MB, ago(400)),
                fe("weird.name.with.dots.7z", 30 * MB, ago(70)),
            ],
        );
        let documents = with_files(leaf("Documents", 0, 0), vec![fe("installer.exe", 50 * MB, ago(500))]);
        let root = drive(vec![parent("Users", vec![parent("bob", vec![downloads, documents])])]);

        let hogs = run(&root, &[]);
        let h = get(&hogs, "old-installers").expect("old-installers");
        assert_eq!(h.safety, Safety::Review);
        let b = "C:\\Users\\bob\\Downloads";
        let want = sorted(vec![
            format!("{b}\\setup.exe"),
            format!("{b}\\old.zip"),
            format!("{b}\\exact-age.exe"),
            format!("{b}\\ubuntu.ISO"),
            format!("{b}\\weird.name.with.dots.7z"),
            format!("{b}\\tools\\tool.msi"),
        ]);
        assert_eq!(sorted_paths(h), want);
        assert_eq!(h.size, (50 + 20 + 10 + 3000 + 30 + 12) * MB);
        assert_eq!(h.count, 6);

        let setup = h.locations.iter().find(|l| l.path.ends_with("setup.exe")).unwrap();
        assert_eq!(setup.note.as_deref(), Some("ดาวน์โหลดไว้ 2 เดือนก่อน"));
        assert_eq!(setup.last_used, Some(ago(60)));
        assert_eq!(setup.ids, vec![0, 0, 0], "ids of the containing Downloads dir");
        let msi = h.locations.iter().find(|l| l.path.ends_with("tool.msi")).unwrap();
        assert_eq!(msi.ids, vec![0, 0, 0, 0]);
        assert_eq!(msi.note.as_deref(), Some("ดาวน์โหลดไว้ 3 เดือนก่อน"));
        let exact = h.locations.iter().find(|l| l.path.ends_with("exact-age.exe")).unwrap();
        assert_eq!(exact.note.as_deref(), Some("ดาวน์โหลดไว้ 1 เดือนก่อน"));
    }

    #[test]
    fn old_installers_cover_every_user_and_nothing_outside_downloads() {
        let user = |n: &str| parent(n, vec![with_files(leaf("Downloads", 0, 0), vec![fe("a.msi", 20 * MB, ago(90))])]);
        let stray = with_files(leaf("Setups", 0, 0), vec![fe("b.msi", 20 * MB, ago(90))]);
        let root = drive(vec![parent("Users", vec![user("a"), user("b")]), stray]);
        let h = get(&run(&root, &[]), "old-installers").cloned().unwrap();
        assert_eq!(sorted_paths(&h), vec!["C:\\Users\\a\\Downloads\\a.msi", "C:\\Users\\b\\Downloads\\a.msi"]);
    }

    #[test]
    fn scanning_a_profile_or_users_folder_directly_still_finds_downloads() {
        let dl = with_files(leaf("Downloads", 0, 0), vec![fe("a.msi", 20 * MB, ago(90))]);
        let profile = parent("bob", vec![dl]);
        let h = find_hogs_with(&profile, Path::new("C:\\Users\\bob"), &[], NOW);
        assert_eq!(paths(get(&h, "old-installers").unwrap()), vec!["C:\\Users\\bob\\Downloads\\a.msi"]);

        let users = parent("Users", vec![profile]);
        let h = find_hogs_with(&users, Path::new("C:\\Users"), &[], NOW);
        assert_eq!(paths(get(&h, "old-installers").unwrap()), vec!["C:\\Users\\bob\\Downloads\\a.msi"]);
    }

    /* ── orphaned AppData ── */

    fn appdata_fixture() -> DirNode {
        let old = ago(200);
        let local = parent(
            "Local",
            vec![
                leaf("OldTool", 60 * MB, old),
                leaf("Recent", 60 * MB, ago(10)),
                leaf("Small", 10 * MB, old),
                leaf("Spotify", 80 * MB, old),
                leaf("Google", 200 * MB, old),
                leaf("Temp", 150 * MB, old),
                leaf("npm-cache", 100 * MB, old),
                leaf("Packages", 500 * MB, old),
                leaf("Boundary89", 60 * MB, ago(89)),
                leaf("Boundary90", 60 * MB, ago(90)),
                leaf("Exact50", 50 * MB, old),
                leaf("JustUnder50", 50 * MB - 1, old),
                with_files(leaf("Wsl", 0, 0), vec![fe("ext4.vhdx", 3000 * MB, old)]),
                leaf("ข้อมูลแอป", 90 * MB, old),
                leaf("Zoom", 90 * MB, old),
                // matched by the dev-caches rule (...\bun\install\cache) but not by name: must not be reported twice
                parent("bun", vec![parent("install", vec![leaf("cache", 120 * MB, old)])]),
            ],
        );
        let roaming = parent("Roaming", vec![leaf("DeadApp", 70 * MB, ago(400)), leaf("Microsoft", 900 * MB, old), leaf("Code", 300 * MB, old)]);
        let locallow = parent("LocalLow", vec![leaf("Ghost", 55 * MB, ago(120))]);
        let programdata =
            parent("ProgramData", vec![leaf("Ghost Soft", 90 * MB, ago(500)), leaf("Microsoft", 400 * MB, old), leaf("Package Cache", 800 * MB, old)]);
        drive(vec![parent("Users", vec![parent("bob", vec![parent("AppData", vec![local, roaming, locallow])])]), programdata])
    }

    #[test]
    fn orphaned_appdata_matched_unmatched_allowlisted() {
        // installed: Spotify, Zoom, VS Code (as the real tokenizer would produce them)
        let hogs = run(&appdata_fixture(), &["spotify", "zoom", "zoomvideocommunications", "microsoftvisualstudiocode", "code"]);
        let h = get(&hogs, "orphaned-appdata").expect("orphaned-appdata");
        assert_eq!(h.safety, Safety::Review);
        assert_eq!(h.label, "ข้อมูลของโปรแกรมที่อาจถอนการติดตั้งแล้ว");
        let l = "C:\\Users\\bob\\AppData";
        let want = sorted(vec![
            format!("{l}\\Local\\OldTool"),
            format!("{l}\\Local\\Boundary90"),
            format!("{l}\\Local\\Exact50"),
            format!("{l}\\Roaming\\DeadApp"),
            format!("{l}\\LocalLow\\Ghost"),
            "C:\\ProgramData\\Ghost Soft".to_string(),
        ]);
        assert_eq!(sorted_paths(h), want);
        assert_eq!(h.size, (60 + 60 + 50 + 70 + 55 + 90) * MB);
        assert_eq!(h.locations[0].path, "C:\\ProgramData\\Ghost Soft", "biggest first");

        let dead = h.locations.iter().find(|l| l.path.ends_with("DeadApp")).unwrap();
        assert_eq!(dead.note.as_deref(), Some("ไม่ได้ใช้งานมา 13 เดือน")); // 400 days
        assert_eq!(dead.last_used, Some(ago(400)));
        // C:\ > Users(0) > bob(0) > AppData(0) > Roaming(1) > DeadApp(0)
        assert_eq!(dead.ids, vec![0, 0, 0, 1, 0]);
        let ghost_soft = h.locations.iter().find(|l| l.path.ends_with("Ghost Soft")).unwrap();
        assert_eq!(ghost_soft.ids, vec![1, 0]);
    }

    #[test]
    fn forgotten_file_inside_orphaned_folder_is_counted_once() {
        let old = ago(500);
        let dead = with_files(leaf("DeadTool", 10 * MB, old), vec![fe("engine.exe", 180 * MB, old)]);
        let root = drive(vec![parent(
            "Users",
            vec![parent("bob", vec![parent("AppData", vec![parent("Local", vec![dead])])])],
        )]);
        let hogs = run(&root, &[]);
        let orphan = get(&hogs, "orphaned-appdata").expect("orphaned");
        assert_eq!(orphan.size, 190 * MB);
        assert!(get(&hogs, "stale-large-files").is_none(), "engine.exe is already inside the orphaned folder");
    }

    #[test]
    fn orphan_detection_skips_installed_allowlisted_recent_small_and_rule_covered_folders() {
        let hogs = run(&appdata_fixture(), &["spotify"]);
        let h = get(&hogs, "orphaned-appdata").unwrap();
        let has = |name: &str| h.locations.iter().any(|l| l.path.ends_with(&format!("\\{name}")));
        assert!(!has("Spotify"), "installed program");
        assert!(!has("Google") && !has("Microsoft") && !has("Packages") && !has("Package Cache"), "allowlisted");
        assert!(!has("Temp") && !has("npm-cache"), "already reported by the temp / dev-caches rules");
        assert!(!has("Wsl"), "already reported through its virtual disk");
        assert!(!has("bun"), "contains a folder already reported by the dev-caches rule");
        assert!(get(&hogs, "dev-caches").unwrap().locations.iter().any(|l| l.path.ends_with("bun\\install\\cache")));
        assert!(!has("Recent") && !has("Boundary89"), "used recently");
        assert!(!has("Small") && !has("JustUnder50"), "too small");
        assert!(!has("ข้อมูลแอป"), "can't be judged, so it is never offered");
        // `Zoom` and `Code` are NOT installed in this run: they are offered
        assert!(has("Zoom") && has("Code"));
    }

    #[test]
    fn orphan_detection_with_unknown_install_list_still_never_offers_system_folders() {
        // nothing installed (e.g. registry unreadable): vendor/system folders are still protected by the allowlist
        let hogs = run(&appdata_fixture(), &[]);
        let h = get(&hogs, "orphaned-appdata").unwrap();
        assert!(h.locations.iter().all(|l| !l.path.ends_with("\\Microsoft") && !l.path.ends_with("\\Google") && !l.path.ends_with("\\Packages")));
    }

    #[test]
    fn partial_name_matches_installed_program() {
        let local = parent("Local", vec![leaf("DiscordPTB", 99 * MB, ago(300)), leaf("UnknownThing", 99 * MB, ago(300))]);
        let root = drive(vec![parent("Users", vec![parent("bob", vec![parent("AppData", vec![local])])])]);
        let h = get(&run(&root, &["discord"]), "orphaned-appdata").cloned().unwrap();
        assert_eq!(paths(&h), vec!["C:\\Users\\bob\\AppData\\Local\\UnknownThing"]);
    }

    /* ── stale large files ── */

    fn stale_files_fixture() -> DirNode {
        let videos = with_files(
            leaf("Videos", 0, 0),
            vec![
                fe("movie.mkv", 500 * MB, ago(400)),
                fe("recent.mkv", 500 * MB, ago(10)),
                fe("almost.mkv", 500 * MB, ago(364)),
                fe("exact.mkv", 100 * MB, ago(365)),
                fe("small.mkv", 100 * MB - 1, ago(900)),
                fe("unknown.mkv", 500 * MB, 0),
                fe("disk.VHDX", 20_000 * MB, ago(900)),
                fe("hiberfil.sys", 6000 * MB, ago(900)),
            ],
        );
        let windows = with_files(leaf("Windows", 0, 0), vec![fe("big.dat", 300 * MB, ago(900))]);
        let pf = parent("Program Files", vec![with_files(leaf("Foo", 0, 0), vec![fe("big.bin", 300 * MB, ago(900))])]);
        let recycle = with_files(leaf("$Recycle.Bin", 0, 0), vec![fe("$R1.iso", 300 * MB, ago(900))]);
        let old_proj = tree(
            "old",
            1,
            ago(300),
            MARKER_JS,
            vec![with_files(leaf("node_modules", 0, 0), vec![fe("binary.node", 200 * MB, ago(800))]), leaf("src", 1, ago(300))],
        );
        let fresh_proj = tree("fresh", 1, ago(5), MARKER_JS, vec![with_files(leaf("node_modules", 0, 0), vec![fe("binary.node", 150 * MB, ago(800))])]);
        let code = parent("code", vec![old_proj, fresh_proj]);
        let mut root = drive(vec![videos, windows, pf, recycle, parent("Users", vec![parent("ann", vec![code])])]);
        root.big_files = vec![fe("pagefile.sys", 8000 * MB, ago(1)), fe("backup.img", 400 * MB, ago(500))];
        root.size += 8400 * MB;
        root
    }

    #[test]
    fn stale_large_files_thresholds_and_exclusions() {
        let hogs = run(&stale_files_fixture(), &[]);
        let h = get(&hogs, "stale-large-files").expect("stale-large-files");
        assert_eq!(h.safety, Safety::Review);
        assert_eq!(h.label, "ไฟล์ใหญ่ที่ไม่ได้แตะเกิน 1 ปี");
        let want = sorted(vec![
            "C:\\Videos\\movie.mkv".to_string(),
            "C:\\Videos\\exact.mkv".to_string(),
            "C:\\backup.img".to_string(),
            "C:\\Users\\ann\\code\\fresh\\node_modules\\binary.node".to_string(),
        ]);
        assert_eq!(
            sorted_paths(h),
            want,
            "not: recent/364d/<100MB/unknown-time, virtual disk, system file, Windows, Program Files, Recycle Bin, claimed node_modules"
        );
        assert_eq!(h.size, (500 + 100 + 400 + 150) * MB);

        let movie = h.locations.iter().find(|l| l.path.ends_with("movie.mkv")).unwrap();
        assert_eq!(movie.note.as_deref(), Some("ไม่ได้แก้ไขมา 13 เดือน")); // 400 days
        assert_eq!(movie.last_used, Some(ago(400)));
        assert_eq!(movie.ids, vec![0], "ids of the containing directory");
        let backup = h.locations.iter().find(|l| l.path.ends_with("backup.img")).unwrap();
        assert!(backup.ids.is_empty(), "root-level file");
        assert_eq!(h.locations[0].size, 500 * MB, "largest first");
    }

    #[test]
    fn stale_files_do_not_overlap_virtual_disks_and_system_files() {
        let hogs = run(&stale_files_fixture(), &[]);
        let vd = get(&hogs, "virtual-disks").unwrap();
        assert_eq!((vd.size, vd.count), (20_000 * MB, 1));
        assert_eq!(vd.locations[0].last_used, Some(ago(900)));
        assert_eq!(get(&hogs, "system-file").map(|h| h.size), Some(8000 * MB));
        let sf = get(&hogs, "stale-large-files").unwrap();
        assert!(!sf.locations.iter().any(|l| l.path.contains("disk.VHDX") || l.path.contains("hiberfil") || l.path.contains("pagefile")));
    }

    #[test]
    fn claimed_projects_exclude_their_files_from_stale_files_but_fresh_ones_stay() {
        let hogs = run(&stale_files_fixture(), &[]);
        let sp = get(&hogs, "stale-projects").unwrap();
        assert_eq!(paths(sp), vec!["C:\\Users\\ann\\code\\old\\node_modules"]);
        let nm = get(&hogs, "node-modules").unwrap();
        assert_eq!(paths(nm), vec!["C:\\Users\\ann\\code\\fresh\\node_modules"]);
    }

    #[test]
    fn stale_large_files_keep_top_50_locations_with_full_totals() {
        let files: Vec<FileEntry> = (0..60u64).map(|i| fe(&format!("f{i}.bin"), (100 + i) * MB, ago(400))).collect();
        let root = drive(vec![with_files(leaf("Archive", 0, 0), files)]);
        let hogs = run(&root, &[]);
        let h = get(&hogs, "stale-large-files").unwrap();
        assert_eq!(h.count, 60);
        assert_eq!(h.size, (0..60u64).map(|i| (100 + i) * MB).sum::<u64>());
        assert_eq!(h.locations.len(), 50);
        assert_eq!(h.locations[0].size, 159 * MB);
        assert_eq!(h.locations[49].size, 110 * MB);
    }

    #[test]
    fn scanning_inside_windows_reports_no_stale_files() {
        let d = with_files(leaf("System32", 0, 0), vec![fe("big.dat", 300 * MB, ago(900))]);
        let hogs = find_hogs_with(&d, Path::new("C:\\Windows\\System32"), &[], NOW);
        assert!(get(&hogs, "stale-large-files").is_none());
    }

    #[test]
    fn unknown_clock_or_future_times_never_look_stale() {
        let root = drive(vec![with_files(leaf("V", 0, 0), vec![fe("future.mkv", 500 * MB, NOW + 10 * DAY)])]);
        assert!(get(&run(&root, &[]), "stale-large-files").is_none());
        let old = drive(vec![with_files(leaf("V", 0, 0), vec![fe("old.mkv", 500 * MB, 1000)])]);
        assert!(get(&find_hogs_with(&old, Path::new("C:\\"), &[], 0), "stale-large-files").is_none(), "now = 0 must not underflow");
    }

    /* ── views ── */

    #[test]
    fn node_view_paths_and_limits() {
        let root = sample();
        let v = node_view(&root, Path::new("C:\\"), &[0, 0], 2).unwrap();
        assert_eq!(v.path, "C:\\Users\\somchai");
        assert_eq!(v.crumbs.len(), 3);
        assert_eq!(v.children.len(), 3, "2 items + 'rest'");
        assert_eq!(v.children[2].kind, "rest");
        assert_eq!(v.children[2].size, 100);
        assert_eq!(v.children[2].newest, 0);
        assert!(node_view(&root, Path::new("C:\\"), &[9], 10).is_none());
    }

    #[test]
    fn node_view_exposes_newest_for_dirs_files_and_small() {
        let mut d = with_files(parent("D", vec![leaf("sub", 500, ago(3))]), vec![fe("big.bin", 1000, ago(8))]);
        d.small_count = 2;
        d.small_size = 10;
        let v = node_view(&d, Path::new("C:\\D"), &[], 10).unwrap();
        let by = |k: &str, n: &str| v.children.iter().find(|c| c.kind == k && c.name.starts_with(n)).unwrap();
        assert_eq!(by("dir", "sub").newest, ago(3));
        assert_eq!(by("file", "big.bin").newest, ago(8));
        assert_eq!(by("small", "ไฟล์").newest, 0);
        let json = serde_json::to_value(&v.children[0]).unwrap();
        assert!(json.get("newest").is_some(), "serialized as `newest`: {json}");
    }

    #[test]
    fn top_files_carry_modified_and_paths() {
        let d = with_files(leaf("docs", 0, 0), vec![fe("a.pdf", 5000, ago(2))]);
        let mut root = drive(vec![d]);
        root.big_files = vec![fe("b.iso", 9000, ago(9))];
        let t = top_files(&root, Path::new("C:\\"), 10);
        assert_eq!((t[0].name.as_str(), t[0].modified, t[0].category), ("b.iso", ago(9), "disk-image"));
        assert_eq!(t[0].path, "C:\\b.iso");
        assert_eq!((t[1].path.as_str(), t[1].modified, t[1].dir_ids.clone()), ("C:\\docs\\a.pdf", ago(2), vec![0]));
        let json = serde_json::to_value(&t[0]).unwrap();
        assert_eq!(json["modified"], ago(9));
    }

    #[test]
    fn location_json_shape_is_snake_case_with_nullable_fields() {
        let l = Location::new("C:\\x".into(), 5, vec![1, 2], 0);
        let j = serde_json::to_value(&l).unwrap();
        assert_eq!(j["last_used"], serde_json::Value::Null);
        assert_eq!(j["note"], serde_json::Value::Null);
        let j = serde_json::to_value(Location::new("C:\\x".into(), 5, vec![], 77).with_note("n".into())).unwrap();
        assert_eq!((j["last_used"].as_u64(), j["note"].as_str()), (Some(77), Some("n")));
    }

    /// Real-world check (read-only): `DISKWHY_BENCH=C:\Users\me cargo test --release real_unused -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_unused_report() {
        use std::sync::atomic::AtomicBool;
        let path = std::env::var("DISKWHY_BENCH").unwrap_or_else(|_| ".".into());
        let (p, c) = (scan::Progress::default(), AtomicBool::new(false));
        let root = scan::scan_dir(Path::new(&path), path.clone().into(), &scan::ScanCtx::new(&p, &c));
        let t = std::time::Instant::now();
        let hogs = find_hogs(&root, Path::new(&path));
        println!("{} installed tokens, find_hogs took {:?}", installed::installed_program_tokens().len(), t.elapsed());
        for h in hogs.iter().filter(|h| ["stale-large-files", "stale-projects", "old-installers", "orphaned-appdata"].contains(&h.id)) {
            println!("== {} ({}): {:.2} GB in {} items", h.id, h.label, h.size as f64 / 1e9, h.count);
            for l in h.locations.iter().take(8) {
                println!("   {:>8.1} MB  {}  [{}]", l.size as f64 / 1e6, l.path, l.note.as_deref().unwrap_or(""));
            }
        }
    }

    #[test]
    fn age_formatting() {
        assert_eq!(months(0), 1);
        assert_eq!(months(29), 1);
        assert_eq!(months(30), 1);
        assert_eq!(months(59), 1);
        assert_eq!(months(60), 2);
        assert_eq!(months(425), 14);
        assert_eq!(age_days(100, 200), 0, "future timestamps");
    }

    #[test]
    fn categorises_extensions() {
        assert_eq!(category_of("mkv"), "video");
        assert_eq!(category_of("vhdx"), "disk-image");
        assert_eq!(category_of("msi"), "installer");
        assert_eq!(category_of("weird"), "other");
    }
}
