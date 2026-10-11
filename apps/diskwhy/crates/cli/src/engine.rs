//! Server state + tool logic: the last finished scan, one background scan, one background cleanup.
//! Everything here is plain Rust (no MCP types) so it can be unit-tested and reused by the CLI.

use crate::fmt;
use crate::views::{self, Reply};
use diskwhy_core::actions::{self, CleanupOutcome, CleanupStatus};
use diskwhy_core::insights::Hog;
use diskwhy_core::report::{self, ScanResult};
use diskwhy_core::safety;
use diskwhy_core::scan::{self, Progress};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

pub const DEFAULT_SCAN_WAIT: u64 = 45;
pub const MAX_SCAN_WAIT: u64 = 55;
/// How long `run_cleanup` waits before answering "still running" (MCP clients time out around 60 s).
pub const CLEANUP_WAIT: Duration = Duration::from_secs(50);
pub const MAX_MOVE_PATHS: usize = 50;

type Recycler = Box<dyn Fn(&Path) -> Result<(), String> + Send + Sync>;
type Cleaner = Arc<dyn Fn(&str) -> Result<CleanupOutcome, String> + Send + Sync>;

struct Finished {
    result: ScanResult,
    /// Cached `find_hogs`; cleared whenever the tree changes.
    hogs: Option<Vec<Hog>>,
}

impl Finished {
    /// The scan plus its findings (computed once, then cached until the tree changes).
    fn parts(&mut self) -> (&ScanResult, &[Hog]) {
        if self.hogs.is_none() {
            self.hogs = Some(diskwhy_core::insights::find_hogs(&self.result.root, &self.result.root_path));
        }
        (&self.result, self.hogs.as_deref().unwrap_or_default())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum JobStatus {
    Running,
    Done,
    Cancelled,
}

struct ScanJob {
    id: u64,
    path: PathBuf,
    progress: Arc<Progress>,
    cancel: Arc<AtomicBool>,
    started: Instant,
    elapsed_at_end: Option<Duration>,
    status: JobStatus,
}

struct CleanupJob {
    action: String,
    started: Instant,
    result: Option<Result<CleanupOutcome, String>>,
}

#[derive(Default)]
struct State {
    last: Option<Finished>,
    job: Option<ScanJob>,
    cleanup: Option<CleanupJob>,
}

pub struct Engine {
    state: Mutex<State>,
    recycler: Recycler,
    cleaner: Cleaner,
    next_id: AtomicU64,
}

/* ── Paths ───────────────────────────────────────────────────────────────── */

/// `\\?\C:\x` -> `C:\x`, `\\?\UNC\srv\share` -> `\\srv\share`.
pub fn strip_verbatim(p: &str) -> String {
    if let Some(rest) = p.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = p.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        p.to_string()
    }
}

/// Tidy a user/model supplied path: trim, unquote, forward slashes to backslashes, collapse repeated separators,
/// "C:" -> "C:\". Rejects control characters, relative paths and any `..` segment.
pub fn normalize_path(raw: &str) -> Result<PathBuf, String> {
    let s = raw.trim().trim_matches(['"', '\'']).trim();
    if s.is_empty() {
        return Err("path ว่างเปล่า".into());
    }
    if s.chars().any(|c| c.is_control()) {
        return Err("path มีอักขระควบคุมที่ไม่อนุญาต".into());
    }
    let mut s = s.replace('/', "\\");
    let unc = s.starts_with(r"\\") && !s.starts_with(r"\\?\");
    while s[usize::from(unc) * 2..].contains("\\\\") {
        let head = &s[..usize::from(unc) * 2];
        let tail = s[usize::from(unc) * 2..].replace("\\\\", "\\");
        s = format!("{head}{tail}");
    }
    if s.len() == 2 && s.ends_with(':') {
        s.push('\\');
    }
    let p = PathBuf::from(&s);
    if !p.is_absolute() {
        return Err(format!("path ต้องเป็น absolute path เช่น C:\\Users\\... (ได้รับ: {raw})"));
    }
    if s.split('\\').any(|seg| seg == "..") {
        return Err("path ห้ามมี \"..\"".into());
    }
    Ok(p)
}

/// Canonical on-disk form without the `\\?\` prefix (resolves 8.3 short names, junctions, symlinks and case).
pub fn canonical_string(p: &Path) -> Option<String> {
    std::fs::canonicalize(p).ok().map(|c| strip_verbatim(&c.to_string_lossy()))
}

fn same_path(a: &str, b: &str) -> bool {
    let n = |s: &str| s.replace('/', "\\").trim_end_matches('\\').to_lowercase();
    n(a) == n(b)
}

fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if !meta.is_dir() {
        return meta.len();
    }
    std::fs::read_dir(path).map(|rd| rd.flatten().map(|e| dir_size(&e.path())).sum()).unwrap_or(0)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn clamp(v: Option<u64>, default: usize, max: usize) -> usize {
    (v.map_or(default, |v| v as usize)).clamp(1, max)
}

const NO_SCAN: &str = "ยังไม่มีผลการสแกน — เรียก scan พร้อม path ของไดรฟ์หรือโฟลเดอร์ก่อน (เช่น \"C:\\\") แล้วรอจนสถานะเป็น done";

/* ── Engine ──────────────────────────────────────────────────────────────── */

impl Engine {
    pub fn new() -> Arc<Self> {
        Self::with_hooks(Box::new(safety::recycle), Arc::new(|a: &str| actions::run_cleanup(a)))
    }

    /// Engine with replaceable side effects (tests inject fakes; production uses [`Engine::new`]).
    pub fn with_hooks(recycler: Recycler, cleaner: Cleaner) -> Arc<Self> {
        Arc::new(Self { state: Mutex::new(State::default()), recycler, cleaner, next_id: AtomicU64::new(1) })
    }

    /// Install a finished scan directly (used by the CLI and tests).
    pub fn set_scan(&self, result: ScanResult) {
        lock(&self.state).last = Some(Finished { result, hogs: None });
    }

    /* scan ------------------------------------------------------------- */

    /// Start (or join) a background scan. Returns true when an already-running scan of the same path was joined.
    pub fn start_scan(self: &Arc<Self>, path: &str) -> Result<bool, String> {
        let given = normalize_path(path)?;
        if !given.is_dir() {
            return Err(if given.exists() {
                format!("{} ไม่ใช่โฟลเดอร์ — ให้ path ของโฟลเดอร์หรือไดรฟ์", given.display())
            } else {
                format!("ไม่พบโฟลเดอร์ {} (ตรวจชื่อไดรฟ์/พาธอีกครั้ง)", given.display())
            });
        }
        let root = PathBuf::from(canonical_string(&given).unwrap_or_else(|| given.to_string_lossy().to_string()));
        let mut st = lock(&self.state);
        if let Some(job) = st.job.as_ref().filter(|j| j.status == JobStatus::Running) {
            if same_path(&job.path.to_string_lossy(), &root.to_string_lossy()) {
                return Ok(true);
            }
            job.cancel.store(true, Relaxed); // a different path: stop the old scan
        }
        let id = self.next_id.fetch_add(1, Relaxed);
        let (progress, cancel) = (Arc::new(Progress::default()), Arc::new(AtomicBool::new(false)));
        st.job = Some(ScanJob {
            id,
            path: root.clone(),
            progress: progress.clone(),
            cancel: cancel.clone(),
            started: Instant::now(),
            elapsed_at_end: None,
            status: JobStatus::Running,
        });
        drop(st);

        let engine = Arc::clone(self);
        std::thread::Builder::new()
            .name("diskwhy-scan".into())
            .spawn(move || {
                let outcome = report::run_scan(&root, &progress, &cancel);
                let mut st = lock(&engine.state);
                let current = st.job.as_ref().is_some_and(|j| j.id == id);
                match outcome {
                    Some(result) if current => {
                        st.last = Some(Finished { result, hogs: None });
                        if let Some(j) = st.job.as_mut() {
                            j.status = JobStatus::Done;
                            j.elapsed_at_end = Some(j.started.elapsed());
                        }
                    }
                    None if current => {
                        if let Some(j) = st.job.as_mut() {
                            j.status = JobStatus::Cancelled;
                            j.elapsed_at_end = Some(j.started.elapsed());
                        }
                    }
                    _ => {} // superseded by a newer scan
                }
            })
            .map_err(|e| format!("เริ่มเธรดสแกนไม่สำเร็จ: {e}"))?;
        Ok(false)
    }

    pub fn is_scan_running(&self) -> bool {
        lock(&self.state).job.as_ref().is_some_and(|j| j.status == JobStatus::Running)
    }

    /// `scan` tool: start/join, wait up to `wait_seconds` (default 45, max 55), then report status.
    pub async fn scan(self: &Arc<Self>, path: &str, wait_seconds: Option<u64>) -> Reply {
        let wait = wait_seconds.unwrap_or(DEFAULT_SCAN_WAIT).min(MAX_SCAN_WAIT);
        let joined = match self.start_scan(path) {
            Ok(j) => j,
            Err(e) => return Reply::err(e),
        };
        let deadline = Instant::now() + Duration::from_secs(wait);
        while self.is_scan_running() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let this = Arc::clone(self);
        let mut reply = tokio::task::spawn_blocking(move || this.scan_status()).await.unwrap_or_else(|_| Reply::err("อ่านสถานะการสแกนไม่สำเร็จ"));
        if joined {
            reply.text = format!("(ต่อจากการสแกน path เดียวกันที่กำลังทำอยู่)\n{}", reply.text);
            reply.data["joined"] = json!(true);
        }
        reply
    }

    pub fn scan_status(&self) -> Reply {
        let mut st = lock(&self.state);
        let State { last, job, .. } = &mut *st;
        if let Some(j) = job.as_ref() {
            match j.status {
                JobStatus::Running => {
                    let p = &j.progress;
                    let current = p.current.try_lock().map(|c| c.clone()).unwrap_or_default();
                    return views::running_reply(&views::ScanSnapshot {
                        path: j.path.to_string_lossy().to_string(),
                        files: p.files.load(Relaxed),
                        dirs: p.dirs.load(Relaxed),
                        bytes: p.bytes.load(Relaxed),
                        errors: p.errors.load(Relaxed),
                        cloud_bytes: p.cloud_bytes.load(Relaxed),
                        current,
                        elapsed_ms: j.started.elapsed().as_millis() as u64,
                    });
                }
                JobStatus::Cancelled => {
                    return Reply::ok(
                        format!("การสแกน {} ถูกยกเลิก (ใช้เวลา {})", j.path.display(), fmt::seconds(j.elapsed_at_end.unwrap_or_default().as_millis() as u64)),
                        json!({ "status": "cancelled", "path": j.path.to_string_lossy() }),
                    );
                }
                JobStatus::Done => {}
            }
        }
        match last {
            Some(f) => {
                let (result, hogs) = f.parts();
                views::done_reply(result, hogs)
            }
            None => views::idle_reply(),
        }
    }

    pub fn cancel_scan(&self) -> Reply {
        let mut st = lock(&self.state);
        match st.job.as_mut().filter(|j| j.status == JobStatus::Running) {
            Some(j) => {
                j.cancel.store(true, Relaxed);
                j.status = JobStatus::Cancelled;
                j.elapsed_at_end = Some(j.started.elapsed());
                let path = j.path.to_string_lossy().to_string();
                Reply::ok(format!("สั่งยกเลิกการสแกน {path} แล้ว"), json!({ "status": "cancelled", "path": path }))
            }
            None => Reply::ok("ไม่มีการสแกนที่กำลังทำงานอยู่", json!({ "status": "idle" })),
        }
    }

    /* read-only views ---------------------------------------------------- */

    fn with_scan<R>(&self, f: impl FnOnce(&mut Finished) -> R) -> Result<R, Reply> {
        let mut st = lock(&self.state);
        let running = st.job.as_ref().is_some_and(|j| j.status == JobStatus::Running);
        match st.last.as_mut() {
            Some(l) => Ok(f(l)),
            None if running => Err(Reply::err("การสแกนยังไม่เสร็จ — เรียก scan_status ซ้ำจนสถานะเป็น done แล้วค่อยเรียกเครื่องมือนี้")),
            None => Err(Reply::err(NO_SCAN)),
        }
    }

    pub fn disk_report(&self, max_findings: Option<u64>, max_locations: Option<u64>) -> Reply {
        let (mf, ml) = (clamp(max_findings, 20, 100), clamp(max_locations, 8, 50));
        self.with_scan(|f| {
            let (result, hogs) = f.parts();
            views::report_reply(result, hogs, mf, ml, fmt::now_secs())
        })
        .unwrap_or_else(|e| e)
    }

    pub fn find_unused(&self) -> Reply {
        self.with_scan(|f| {
            let (result, hogs) = f.parts();
            views::unused_reply(result, hogs, 30, fmt::now_secs())
        })
        .unwrap_or_else(|e| e)
    }

    pub fn browse(&self, path: Option<&str>, limit: Option<u64>) -> Reply {
        let limit = clamp(limit, 40, 200);
        self.with_scan(|f| views::browse_reply(&f.result, path, limit, fmt::now_secs())).unwrap_or_else(|e| e)
    }

    pub fn largest_files(&self, limit: Option<u64>, min_size_mb: Option<f64>, category: Option<&str>) -> Reply {
        let limit = clamp(limit, 30, 200);
        self.with_scan(|f| views::largest_reply(&f.result, limit, min_size_mb, category, fmt::now_secs())).unwrap_or_else(|e| e)
    }

    pub fn admin_commands(&self) -> Reply {
        let ctx = self.with_scan(|f| views::admin_context_from(f.parts().1)).unwrap_or_default();
        views::admin_commands_reply(&ctx)
    }

    /* move_to_recycle_bin ------------------------------------------------ */

    pub fn move_to_recycle_bin(&self, paths: &[String], reason: Option<&str>) -> Reply {
        if paths.is_empty() {
            return Reply::err("ต้องระบุ paths อย่างน้อย 1 รายการ");
        }
        if paths.len() > MAX_MOVE_PATHS {
            return Reply::err(format!("ส่ง paths ได้ครั้งละไม่เกิน {MAX_MOVE_PATHS} รายการ (ได้รับ {})", paths.len()));
        }
        let scan_root = lock(&self.state).last.as_ref().map(|f| f.result.root_path.clone());
        let mut warnings = Vec::new();
        if scan_root.is_none() {
            warnings.push("ยังไม่มีผลการสแกน จึงตรวจไม่ได้ว่าอยู่ในขอบเขตที่สแกน — ตรวจพาธให้แน่ใจก่อน".to_string());
        }
        let free_before = paths.first().and_then(|p| normalize_path(p).ok()).and_then(|p| report::drive_for(&p)).map(|d| d.available);

        let mut seen: Vec<String> = Vec::new();
        let mut results: Vec<Value> = Vec::new();
        let (mut moved, mut moved_bytes, mut failed) = (0u64, 0u64, 0u64);
        for raw in paths {
            let r = self.move_one(raw, scan_root.as_deref(), &mut seen);
            match r.status {
                "moved" => {
                    moved += 1;
                    moved_bytes += r.bytes;
                }
                _ => failed += 1,
            }
            results.push(json!({ "path": r.path, "status": r.status, "message": r.message, "bytes": r.bytes, "size": fmt::bytes(r.bytes) }));
        }
        let free_after = paths.first().and_then(|p| normalize_path(p).ok()).and_then(|p| report::drive_for(&p)).map(|d| d.available);

        let mut text = format!("ย้ายไปถังขยะสำเร็จ {moved} จาก {} รายการ รวม {}", paths.len(), fmt::bytes(moved_bytes));
        if let Some(r) = reason.filter(|r| !r.trim().is_empty()) {
            text.push_str(&format!(" (เหตุผล: {})", fmt::shorten(r.trim(), 200)));
        }
        text.push('\n');
        for r in &results {
            text.push_str(&format!("- [{}] {} — {}\n", r["status"].as_str().unwrap_or_default(), r["path"].as_str().unwrap_or_default(), r["message"].as_str().unwrap_or_default()));
        }
        for w in &warnings {
            text.push_str(&format!("คำเตือน: {w}\n"));
        }
        if moved > 0 {
            text.push_str("หมายเหตุ: ไฟล์ยังอยู่ในถังขยะ (กู้คืนได้) พื้นที่ว่างของไดรฟ์จะเพิ่มก็ต่อเมื่อล้างถังขยะ (run_cleanup empty-recycle-bin ซึ่งลบถาวร ต้องถามผู้ใช้ก่อน)");
        }
        Reply {
            is_error: moved == 0,
            text: text.trim_end().to_string(),
            data: json!({
                "movedCount": moved,
                "failedCount": failed,
                "movedBytes": moved_bytes,
                "moved": fmt::bytes(moved_bytes),
                "results": results,
                "warnings": warnings,
                "reason": reason,
                "recoverableFromRecycleBin": true,
                "driveFreeBeforeBytes": free_before,
                "driveFreeAfterBytes": free_after,
                "spaceReturnsAfterEmptyingRecycleBin": true,
            }),
        }
    }

    fn move_one(&self, raw: &str, scan_root: Option<&Path>, seen: &mut Vec<String>) -> MoveResult {
        let refuse = |path: &str, msg: String| MoveResult { path: path.to_string(), status: "refused", message: msg, bytes: 0 };
        let given = match normalize_path(raw) {
            Ok(p) => p,
            Err(e) => return refuse(raw, e),
        };
        let shown = given.to_string_lossy().to_string();
        if seen.iter().any(|s| same_path(s, &shown)) {
            return MoveResult { path: shown, status: "refused", message: "ซ้ำกับรายการก่อนหน้าในคำขอเดียวกัน".into(), bytes: 0 };
        }
        seen.push(shown.clone());

        if safety::is_protected(&given) {
            return refuse(&shown, safety::PROTECTED_MESSAGE.to_string());
        }
        if std::fs::symlink_metadata(&given).is_err() {
            return MoveResult {
                path: shown,
                status: "not-found",
                message: "ไม่พบไฟล์หรือโฟลเดอร์ (อาจถูกย้ายไปพร้อมโฟลเดอร์แม่ในคำขอเดียวกันแล้ว)".into(),
                bytes: 0,
            };
        }
        // Resolve short names / junctions / symlinks so the protection check cannot be dodged (PROGRA~1, links into C:\Windows).
        let Some(canon) = canonical_string(&given) else {
            return refuse(&shown, "ตรวจตำแหน่งจริงของ path ไม่ได้ จึงไม่ย้ายเพื่อความปลอดภัย".into());
        };
        if safety::is_protected(Path::new(&canon)) {
            return refuse(&shown, safety::PROTECTED_MESSAGE.to_string());
        }
        if !same_path(&canon, &shown) {
            return refuse(&shown, format!("path นี้ชี้ไปที่ตำแหน่งอื่น ({canon}) เช่น ชื่อสั้น 8.3 / junction / symlink — ให้ใช้พาธจริง"));
        }
        if let Some(root) = scan_root {
            if !views::path_is_within(root, &canon) {
                return refuse(&shown, format!("อยู่นอกขอบเขตที่สแกนล่าสุด ({}) — สแกนโฟลเดอร์ที่ครอบคลุม path นี้ก่อน", root.display()));
            }
            if same_path(&root.to_string_lossy(), &canon) {
                return refuse(&shown, "นี่คือโฟลเดอร์รากของการสแกน — เลือกโฟลเดอร์ย่อยแทน".into());
            }
        }

        let in_tree = lock(&self.state).last.as_ref().and_then(|f| tree_size(&f.result, &canon));
        let bytes = in_tree.unwrap_or_else(|| dir_size(&given));
        if let Err(e) = (self.recycler)(&given) {
            return MoveResult { path: shown, status: "failed", message: e, bytes: 0 };
        }
        // Keep the in-memory tree in sync.
        let mut st = lock(&self.state);
        if let Some(f) = st.last.as_mut() {
            if let Some((ids, file)) = safety::locate(&f.result.root_path, &f.result.root, &canon) {
                if scan::remove_from_tree(&mut f.result.root, &ids, file.as_deref()).is_some() {
                    f.hogs = None;
                    f.result.stats.bytes = f.result.root.size;
                    f.result.stats.files = f.result.root.files;
                    f.result.stats.dirs = f.result.root.dirs;
                }
            }
        }
        MoveResult { path: shown, status: "moved", message: "ย้ายไปถังขยะแล้ว (กู้คืนได้จาก Recycle Bin)".into(), bytes }
    }

    /* run_cleanup -------------------------------------------------------- */

    /// Start (or join) a background cleanup. Returns (joined, finished result already waiting).
    pub fn start_cleanup(self: &Arc<Self>, action: &str) -> Result<bool, String> {
        let action = action.trim();
        let spec = actions::cleanup_spec(action)
            .ok_or_else(|| format!("ไม่รองรับ action {action:?} — ใช้ได้เฉพาะ: {}", actions::cleanup_ids().join(", ")))?;
        let mut st = lock(&self.state);
        if let Some(job) = st.cleanup.as_ref() {
            if job.action == spec.id {
                return Ok(true); // running, or finished and not yet collected
            }
            if job.result.is_none() {
                return Err(format!("กำลังรัน {} อยู่ — รอให้เสร็จก่อน (เรียก run_cleanup {} ซ้ำเพื่อดูผล)", job.action, job.action));
            }
        }
        st.cleanup = Some(CleanupJob { action: spec.id.to_string(), started: Instant::now(), result: None });
        drop(st);
        let (engine, id) = (Arc::clone(self), spec.id);
        std::thread::Builder::new()
            .name("diskwhy-cleanup".into())
            .spawn(move || {
                let r = (engine.cleaner)(id);
                if let Some(j) = lock(&engine.state).cleanup.as_mut().filter(|j| j.action == id && j.result.is_none()) {
                    j.result = Some(r);
                }
            })
            .map_err(|e| format!("เริ่มงานล้างไม่สำเร็จ: {e}"))?;
        Ok(false)
    }

    pub async fn run_cleanup(self: &Arc<Self>, action: &str, wait: Duration) -> Reply {
        let joined = match self.start_cleanup(action) {
            Ok(j) => j,
            Err(e) => return Reply::err(e),
        };
        let action = action.trim().to_string();
        let deadline = Instant::now() + wait;
        loop {
            {
                let mut st = lock(&self.state);
                if let Some(job) = st.cleanup.as_mut().filter(|j| j.action == action) {
                    if let Some(res) = job.result.take() {
                        st.cleanup = None;
                        return cleanup_reply(&action, res, joined);
                    }
                    if Instant::now() >= deadline {
                        let secs = job.started.elapsed().as_secs();
                        return Reply::ok(
                            format!("{action} ยังทำงานอยู่ (ผ่านไป {secs} วินาที) — เรียก run_cleanup ด้วย action เดิมซ้ำเพื่อรอผลลัพธ์ (ไม่เริ่มงานซ้ำ)"),
                            json!({ "status": "running", "action": action, "elapsedSeconds": secs, "next": "call run_cleanup again with the same action to collect the result" }),
                        );
                    }
                } else {
                    return Reply::err("งานล้างหายไปจากสถานะภายใน — ลองเรียกใหม่อีกครั้ง");
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
}

struct MoveResult {
    path: String,
    status: &'static str,
    message: String,
    bytes: u64,
}

/// Size of `target` according to the scanned tree (None when it is not in the tree).
fn tree_size(r: &ScanResult, target: &str) -> Option<u64> {
    let (ids, file) = safety::locate(&r.root_path, &r.root, target)?;
    let mut node = &r.root;
    for &i in &ids {
        node = node.children.get(i as usize)?;
    }
    match file {
        Some(name) => node.big_files.iter().find(|f| *f.name == *name).map(|f| f.size),
        None => Some(node.size),
    }
}

pub fn cleanup_reply(action: &str, res: Result<CleanupOutcome, String>, from_earlier: bool) -> Reply {
    let o = match res {
        Ok(o) => o,
        Err(e) => return Reply::err(e),
    };
    let title = actions::cleanup_spec(action).map_or(action, |s| s.title);
    let status = match o.status {
        CleanupStatus::Done => "สำเร็จ",
        CleanupStatus::NotInstalled => "ไม่พบเครื่องมือนี้ในเครื่อง",
        CleanupStatus::Failed => "ไม่สำเร็จ",
        CleanupStatus::Timeout => "หมดเวลา",
        CleanupStatus::Opened => "เปิดหน้าต่างแล้ว",
    };
    let mut text = format!("{title} ({action}): {status}\n{}", o.message);
    if let Some(c) = &o.command {
        text.push_str(&format!("\nคำสั่ง: {c}"));
    }
    if let (Some(b), Some(a)) = (o.free_before, o.free_after) {
        text.push_str(&format!("\nพื้นที่ว่างไดรฟ์ระบบ: {} -> {} (คืนเพิ่ม {})", fmt::bytes(b), fmt::bytes(a), fmt::bytes(o.freed_bytes)));
    }
    if o.items_moved + o.items_skipped > 0 {
        text.push_str(&format!(
            "\nย้ายไปถังขยะ {} รายการ ({}) ข้าม {} — พื้นที่จะคืนหลังล้างถังขยะ",
            o.items_moved,
            fmt::bytes(o.moved_to_recycle_bytes),
            o.items_skipped
        ));
    }
    if !o.stdout_tail.is_empty() {
        text.push_str(&format!("\n--- stdout ---\n{}", o.stdout_tail));
    }
    if !o.stderr_tail.is_empty() {
        text.push_str(&format!("\n--- stderr ---\n{}", o.stderr_tail));
    }
    if from_earlier {
        text.push_str("\n(ผลของงานนี้ที่เริ่มไว้ก่อนหน้า)");
    }
    let mut data = serde_json::to_value(&o).unwrap_or(Value::Null);
    data["freed"] = json!(fmt::bytes(o.freed_bytes));
    Reply { is_error: matches!(o.status, CleanupStatus::Failed | CleanupStatus::Timeout), text, data }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn canon_dir() -> (tempfile::TempDir, String) {
        let d = tempfile::tempdir().unwrap();
        let c = canonical_string(d.path()).unwrap();
        (d, c)
    }

    /// Engine whose recycler really deletes (inside a tempdir) instead of using the user's Recycle Bin.
    fn fake_engine(calls: Arc<AtomicUsize>) -> Arc<Engine> {
        Engine::with_hooks(
            Box::new(move |p: &Path| {
                calls.fetch_add(1, Relaxed);
                if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) }.map_err(|e| e.to_string())
            }),
            Arc::new(|a: &str| Err(format!("tests must not run cleanups: {a}"))),
        )
    }

    fn fixture(root: &str) {
        let r = Path::new(root);
        std::fs::create_dir_all(r.join("proj/node_modules/lib")).unwrap();
        std::fs::write(r.join("proj/package.json"), b"{}").unwrap();
        std::fs::write(r.join("proj/node_modules/lib/big.bin"), vec![1u8; 3 * 1024 * 1024]).unwrap();
        std::fs::write(r.join("movie.mp4"), vec![2u8; 2 * 1024 * 1024]).unwrap();
        std::fs::write(r.join("small.txt"), b"x").unwrap();
    }

    #[tokio::test]
    async fn scan_lifecycle_wait_and_status() {
        let (_d, root) = canon_dir();
        fixture(&root);
        let e = fake_engine(Arc::default());
        assert_eq!(e.scan_status().data["status"], "idle");
        assert!(e.disk_report(None, None).is_error, "no scan yet");

        let r = e.scan(&root, Some(30)).await;
        assert!(!r.is_error, "{}", r.text);
        assert_eq!(r.data["status"], "done");
        assert_eq!(r.data["files"], 4);
        assert_eq!(r.data["totalBytes"], 5 * 1024 * 1024 + 2 + 1);
        assert_eq!(r.data["unreadable"], 0);
        assert!(r.data["topFindings"].as_array().unwrap().len() <= 5);
        assert!(!e.is_scan_running());
        // scan_status returns the same finished data without rescanning
        assert_eq!(e.scan_status().data["totalBytes"], r.data["totalBytes"]);
        // wait_seconds = 0 returns immediately with a valid shape
        let again = e.scan(&root, Some(0)).await;
        assert!(["running", "done"].contains(&again.data["status"].as_str().unwrap()));
    }

    #[tokio::test]
    async fn scan_validates_paths() {
        let (_d, root) = canon_dir();
        fixture(&root);
        let e = fake_engine(Arc::default());
        let missing = e.scan(&format!("{root}\\does-not-exist"), Some(1)).await;
        assert!(missing.is_error && missing.text.contains("ไม่พบ"));
        let file = e.scan(&format!("{root}\\movie.mp4"), Some(1)).await;
        assert!(file.is_error && file.text.contains("ไม่ใช่โฟลเดอร์"));
        assert!(e.scan("relative\\dir", Some(1)).await.is_error);
        assert!(e.scan("", Some(1)).await.is_error);
        assert!(e.scan(&format!("{root}\\..\\x"), Some(1)).await.is_error);
    }

    #[test]
    fn running_scan_is_joined_and_a_different_one_cancels_it() {
        let e = fake_engine(Arc::default());
        // Fake a running job so the test needs no big directory.
        let (_d, root) = canon_dir();
        let (_d2, other) = canon_dir();
        {
            let mut st = lock(&e.state);
            st.job = Some(ScanJob {
                id: 99,
                path: PathBuf::from(&root),
                progress: Arc::new(Progress::default()),
                cancel: Arc::new(AtomicBool::new(false)),
                started: Instant::now(),
                elapsed_at_end: None,
                status: JobStatus::Running,
            });
        }
        let running = e.scan_status();
        assert_eq!(running.data["status"], "running");
        assert!(running.text.contains("scan_status"));
        // same path (different case / slashes) joins the running job
        let joined = e.start_scan(&root.to_uppercase().replace('\\', "/")).unwrap();
        assert!(joined);
        let flag = lock(&e.state).job.as_ref().unwrap().cancel.clone();
        assert!(!flag.load(Relaxed));
        // a different path cancels the old job and starts a new one
        assert!(!e.start_scan(&other).unwrap());
        assert!(flag.load(Relaxed), "old scan was cancelled");
        assert_ne!(lock(&e.state).job.as_ref().unwrap().id, 99);
        // cancel_scan stops the new job
        let c = e.cancel_scan();
        assert!(matches!(c.data["status"].as_str(), Some("cancelled") | Some("idle")));
    }

    #[tokio::test]
    async fn report_browse_largest_and_unused_need_a_scan_then_work() {
        let (_d, root) = canon_dir();
        fixture(&root);
        let e = fake_engine(Arc::default());
        for r in [e.find_unused(), e.browse(None, None), e.largest_files(None, None, None), e.disk_report(None, None)] {
            assert!(r.is_error && r.text.contains("scan"));
        }
        e.scan(&root, Some(30)).await;
        let rep = e.disk_report(Some(5), Some(2));
        assert!(!rep.is_error);
        assert!(rep.data["findings"].as_array().unwrap().len() <= 5);
        assert!(rep.data["categories"].as_array().unwrap().iter().any(|c| c["category"] == "video"));
        assert_eq!(e.browse(None, None).data["children"][0]["name"], "proj");
        assert_eq!(e.largest_files(Some(1), None, None).data["files"][0]["name"], "big.bin");
        assert_eq!(e.largest_files(None, Some(2.5), None).data["totalMatching"], 1);
        assert_eq!(e.largest_files(None, None, Some("video")).data["files"][0]["name"], "movie.mp4");
        assert!(e.largest_files(None, None, Some("bogus")).is_error);
        assert_eq!(e.find_unused().data["findings"].as_array().map(|a| a.len() <= 4), Some(true));
        // limits are clamped, not rejected
        assert!(!e.disk_report(Some(0), Some(10_000)).is_error);
    }

    #[tokio::test]
    async fn admin_commands_work_with_and_without_a_scan() {
        let e = fake_engine(Arc::default());
        let no_scan = e.admin_commands();
        assert!(!no_scan.is_error);
        assert!(no_scan.text.contains("DISM /Online /Cleanup-Image /StartComponentCleanup"));
        assert!(!serde_json::to_string(&no_scan.data).unwrap().contains("Optimize-VHD"));
        let (_d, root) = canon_dir();
        fixture(&root);
        std::fs::write(Path::new(&root).join("disk.vhdx"), vec![0u8; 2 * 1024 * 1024]).unwrap();
        e.scan(&root, Some(30)).await;
        let with = e.admin_commands();
        let json = serde_json::to_string(&with.data).unwrap();
        assert!(json.contains("Optimize-VHD") && json.contains("disk.vhdx"), "{json}");
        assert_eq!(with.data["executedByDiskWhy"], false);
    }

    #[tokio::test]
    async fn move_to_recycle_bin_updates_tree_and_refuses_unsafe_paths() {
        let (_d, root) = canon_dir();
        fixture(&root);
        let calls = Arc::new(AtomicUsize::new(0));
        let e = fake_engine(calls.clone());
        e.scan(&root, Some(30)).await;
        let before = e.scan_status().data["totalBytes"].as_u64().unwrap();

        // 1) protected system paths: refused, recycler never called
        let prot = e.move_to_recycle_bin(&["C:\\Windows\\System32".into(), "C:\\".into(), "C:\\Program Files\\x".into(), "C:\\Users".into()], None);
        assert!(prot.is_error);
        assert!(prot.data["results"].as_array().unwrap().iter().all(|r| r["status"] == "refused"));
        assert!(prot.text.contains("ระบบ"));
        // 2) `..` traversal and relative paths are refused
        let trav = e.move_to_recycle_bin(&[format!("{root}\\proj\\..\\..\\Windows"), "movie.mp4".into()], None);
        assert!(trav.data["results"].as_array().unwrap().iter().all(|r| r["status"] == "refused"));
        // 3) outside the scan root: refused
        let (_other, other_root) = canon_dir();
        std::fs::write(Path::new(&other_root).join("x.bin"), b"1").unwrap();
        let outside = e.move_to_recycle_bin(&[format!("{other_root}\\x.bin")], None);
        assert!(outside.is_error && outside.text.contains("นอกขอบเขต"), "{}", outside.text);
        assert!(Path::new(&other_root).join("x.bin").exists());
        // 4) the scan root itself is refused
        assert!(e.move_to_recycle_bin(&[root.clone()], None).is_error);
        // 5) empty / too many
        assert!(e.move_to_recycle_bin(&[], None).is_error);
        assert!(e.move_to_recycle_bin(&vec!["C:\\x".to_string(); 51], None).is_error);
        assert_eq!(calls.load(Relaxed), 0, "nothing unsafe reached the recycler");

        // 6) a legitimate move: folder + a file inside it (second one is already gone) + a file elsewhere
        let nm = format!("{root}\\proj\\node_modules");
        let movie = format!("{root}\\movie.mp4");
        let ok = e.move_to_recycle_bin(&[nm.clone(), format!("{nm}\\lib\\big.bin"), movie.clone(), format!("{root}\\nope.bin")], Some("regenerable"));
        assert!(!ok.is_error, "{}", ok.text);
        assert_eq!(ok.data["movedCount"], 2);
        assert_eq!(ok.data["movedBytes"], 5 * 1024 * 1024);
        let statuses: Vec<_> = ok.data["results"].as_array().unwrap().iter().map(|r| r["status"].as_str().unwrap().to_string()).collect();
        assert_eq!(statuses, ["moved", "not-found", "moved", "not-found"]);
        assert_eq!(calls.load(Relaxed), 2);
        assert!(!Path::new(&nm).exists() && !Path::new(&movie).exists());
        assert!(ok.text.contains("ถังขยะ"));
        assert_eq!(ok.data["recoverableFromRecycleBin"], true);
        // the in-memory tree was updated
        let after = e.scan_status().data["totalBytes"].as_u64().unwrap();
        assert_eq!(before - after, 5 * 1024 * 1024);
        assert_eq!(e.browse(None, None).data["children"].as_array().unwrap().len(), 2, "proj + small files tile");
        // duplicate paths in one request are not processed twice
        let dup = e.move_to_recycle_bin(&[format!("{root}\\small.txt"), format!("{root}\\SMALL.txt")], None);
        let st: Vec<_> = dup.data["results"].as_array().unwrap().iter().map(|r| r["status"].as_str().unwrap().to_string()).collect();
        assert_eq!(st, ["moved", "refused"]);
    }

    #[test]
    fn move_without_a_scan_is_allowed_with_a_warning() {
        let (_d, root) = canon_dir();
        std::fs::write(Path::new(&root).join("a.bin"), vec![0u8; 10]).unwrap();
        let e = fake_engine(Arc::default());
        let r = e.move_to_recycle_bin(&[format!("{root}\\a.bin")], None);
        assert!(!r.is_error, "{}", r.text);
        assert_eq!(r.data["warnings"].as_array().unwrap().len(), 1);
        assert!(r.text.contains("คำเตือน"));
    }

    #[test]
    fn short_names_and_links_cannot_dodge_the_protection_check() {
        let e = fake_engine(Arc::default());
        // A path that resolves elsewhere (here: different case is fine, but 8.3 names must be refused or not found).
        let r = e.move_to_recycle_bin(&["C:\\PROGRA~1\\anything".into()], None);
        assert!(r.is_error);
        assert_ne!(r.data["results"][0]["status"], "moved");
    }

    #[test]
    fn path_normalisation() {
        assert_eq!(normalize_path(" \"C:/Users/a//b/\" ").unwrap(), PathBuf::from("C:\\Users\\a\\b\\"));
        assert_eq!(normalize_path("C:").unwrap(), PathBuf::from("C:\\"));
        assert!(normalize_path("C:\\a\\..\\b").is_err());
        assert!(normalize_path("a\\b").is_err());
        assert!(normalize_path("C:\\a\u{0}b").is_err());
        assert_eq!(strip_verbatim(r"\\?\C:\x"), r"C:\x");
        assert_eq!(strip_verbatim(r"\\?\UNC\srv\sh\x"), r"\\srv\sh\x");
    }

    #[tokio::test]
    async fn run_cleanup_rejects_unknown_actions_and_uses_the_injected_cleaner() {
        let e = Engine::with_hooks(
            Box::new(|_| Ok(())),
            Arc::new(|a: &str| {
                std::thread::sleep(Duration::from_millis(300));
                let spec = actions::cleanup_spec(a).ok_or("unknown")?;
                Ok(CleanupOutcome {
                    action: spec.id.into(),
                    status: CleanupStatus::Done,
                    message: "ok".into(),
                    command: Some("fake".into()),
                    exit_code: Some(0),
                    stdout_tail: String::new(),
                    stderr_tail: String::new(),
                    free_before: Some(100),
                    free_after: Some(1100),
                    freed_bytes: 1000,
                    moved_to_recycle_bytes: 0,
                    items_moved: 0,
                    items_skipped: 0,
                })
            }),
        );
        for bad in ["", "rm -rf /", "npm-cache; calc", "NPM-CACHE"] {
            let r = e.run_cleanup(bad, Duration::from_secs(1)).await;
            assert!(r.is_error && r.text.contains("ไม่รองรับ"), "{bad:?}: {}", r.text);
        }
        // finishes within the wait
        let done = e.run_cleanup("npm-cache", Duration::from_secs(10)).await;
        assert!(!done.is_error, "{}", done.text);
        assert_eq!(done.data["status"], "done");
        assert_eq!(done.data["freedBytes"], 1000);
        assert!(done.text.contains("คืนเพิ่ม"));
        // slow job: first call reports "running", a second call with the same action joins and collects it
        let running = e.run_cleanup("yarn-cache", Duration::from_millis(10)).await;
        assert_eq!(running.data["status"], "running");
        let other = e.run_cleanup("pnpm-store", Duration::from_millis(10)).await;
        assert!(other.is_error && other.text.contains("yarn-cache"), "only one cleanup at a time: {}", other.text);
        let collected = e.run_cleanup("yarn-cache", Duration::from_secs(10)).await;
        assert_eq!(collected.data["status"], "done");
        assert_eq!(collected.data["action"], "yarn-cache");
    }

    #[test]
    fn cleanup_reply_shapes() {
        let mut o = CleanupOutcome {
            action: "pip-cache".into(),
            status: CleanupStatus::NotInstalled,
            message: "ไม่พบเครื่องมือนี้ในเครื่อง".into(),
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
        };
        let nf = cleanup_reply("pip-cache", Ok(o.clone()), false);
        assert!(!nf.is_error, "not installed is a helpful message, not an error");
        assert_eq!(nf.data["status"], "not-installed");
        o.status = CleanupStatus::Failed;
        assert!(cleanup_reply("pip-cache", Ok(o), false).is_error);
        assert!(cleanup_reply("x", Err("boom".into()), false).is_error);
    }
}
