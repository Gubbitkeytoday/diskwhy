//! Pure "view" functions: turn core results into a [`Reply`] (Thai text for humans + JSON for machines).
//! Shared by the MCP tools (text + structuredContent) and the CLI (`--json` prints `data`, otherwise `text`).

use crate::fmt;
use diskwhy_core::insights::{self, Hog, Location, Safety};
use diskwhy_core::report::{self, DriveInfo, ScanResult};
use diskwhy_core::safety;
use serde_json::{Value, json};
use std::path::Path;

/// A tool / command result.
#[derive(Debug, Clone)]
pub struct Reply {
    pub text: String,
    pub data: Value,
    pub is_error: bool,
}

impl Reply {
    pub fn ok(text: impl Into<String>, data: Value) -> Self {
        Self { text: text.into(), data, is_error: false }
    }

    pub fn err(message: impl Into<String>) -> Self {
        let message = message.into();
        Self { data: json!({ "error": message }), text: message, is_error: true }
    }
}

/// File categories produced by `insights::category_of`.
pub const CATEGORIES: &[&str] =
    &["video", "image", "audio", "archive", "disk-image", "installer", "document", "system", "temp-log", "code", "game", "database", "other"];

/// The findings that describe stale / unused / orphaned data.
pub const UNUSED_IDS: &[&str] = &["stale-large-files", "stale-projects", "old-installers", "orphaned-appdata"];

pub fn safety_str(s: Safety) -> &'static str {
    match s {
        Safety::Safe => "safe",
        Safety::Review => "review",
        Safety::System => "system",
    }
}

fn safety_label(s: Safety) -> &'static str {
    match s {
        Safety::Safe => "safe = ปลอดภัย สร้างคืนได้",
        Safety::Review => "review = ตรวจกับผู้ใช้ก่อน",
        Safety::System => "system = Windows จัดการ ห้ามลบเอง",
    }
}

/* ── Drives ──────────────────────────────────────────────────────────────── */

pub const NEARLY_FULL_PERCENT: f64 = 90.0;

pub fn drive_json(d: &DriveInfo) -> Value {
    let used = d.total.saturating_sub(d.available);
    let used_percent = fmt::percent(used, d.total);
    json!({
        "mount": d.mount,
        "label": d.name,
        "fileSystem": d.file_system,
        "totalBytes": d.total,
        "freeBytes": d.available,
        "usedBytes": used,
        "total": fmt::bytes(d.total),
        "free": fmt::bytes(d.available),
        "used": fmt::bytes(used),
        "usedPercent": used_percent,
        "removable": d.removable,
        "nearlyFull": used_percent >= NEARLY_FULL_PERCENT,
    })
}

pub fn drives_reply() -> Reply {
    drives_reply_from(&report::drives())
}

pub fn drives_reply_from(list: &[DriveInfo]) -> Reply {
    if list.is_empty() {
        return Reply::err("ไม่พบไดรฟ์ที่อ่านข้อมูลได้");
    }
    let mut text = format!("ไดรฟ์ในเครื่อง ({} ไดรฟ์)\n", list.len());
    let mut items = Vec::new();
    for d in list {
        let j = drive_json(d);
        let flag = if j["nearlyFull"] == true { "  ** เกือบเต็ม (ใช้ไป 90% ขึ้นไป) **" } else { "" };
        let label = if d.name.is_empty() { String::new() } else { format!(" ({})", d.name) };
        text.push_str(&format!(
            "- {}{}  {}{}  ใช้ {} / {} ({}%)  ว่าง {}{}\n",
            d.mount,
            label,
            d.file_system,
            if d.removable { " ถอดได้" } else { "" },
            j["used"].as_str().unwrap_or_default(),
            j["total"].as_str().unwrap_or_default(),
            j["usedPercent"],
            j["free"].as_str().unwrap_or_default(),
            flag,
        ));
        items.push(j);
    }
    Reply::ok(text.trim_end(), json!({ "drives": items }))
}

/* ── Scan status ─────────────────────────────────────────────────────────── */

#[derive(Debug, Clone)]
pub struct ScanSnapshot {
    pub path: String,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub errors: u64,
    pub cloud_bytes: u64,
    pub current: String,
    pub elapsed_ms: u64,
}

pub fn running_reply(s: &ScanSnapshot) -> Reply {
    let text = format!(
        "กำลังสแกน {} ... พบแล้ว {} ไฟล์ ({}) ใช้เวลา {}\nกำลังอ่าน: {}\nเรียก scan_status ซ้ำเพื่อดูความคืบหน้า (สแกนทั้งไดรฟ์ C: ใช้เวลา 1-3 นาที) แล้วค่อยเรียก disk_report",
        s.path,
        s.files,
        fmt::bytes(s.bytes),
        fmt::seconds(s.elapsed_ms),
        fmt::shorten(&s.current, 90),
    );
    Reply::ok(
        text,
        json!({
            "status": "running",
            "path": s.path,
            "files": s.files,
            "dirs": s.dirs,
            "bytes": s.bytes,
            "size": fmt::bytes(s.bytes),
            "current": s.current,
            "elapsedMs": s.elapsed_ms,
            "elapsed": fmt::seconds(s.elapsed_ms),
            "next": "call scan_status again until status is done"
        }),
    )
}

pub fn idle_reply() -> Reply {
    Reply::ok(
        "ยังไม่มีผลการสแกน — เรียก scan พร้อม path ของไดรฟ์หรือโฟลเดอร์ (เช่น \"C:\\\") ก่อน",
        json!({ "status": "idle", "next": "call scan with a path first" }),
    )
}

pub fn finding_summary_json(h: &Hog) -> Value {
    json!({
        "id": h.id,
        "label": h.label,
        "safety": safety_str(h.safety),
        "bytes": h.size,
        "size": fmt::bytes(h.size),
        "count": h.count,
    })
}

pub fn done_reply(r: &ScanResult, hogs: &[Hog]) -> Reply {
    let top: Vec<Value> = hogs.iter().take(5).map(finding_summary_json).collect();
    let mut text = format!(
        "สแกนเสร็จแล้ว: {}\nรวม {} ใน {} ไฟล์ / {} โฟลเดอร์ (ใช้เวลา {}), อ่านไม่ได้ {} โฟลเดอร์, ไฟล์ OneDrive แบบออนไลน์อย่างเดียว {} (ไม่กินพื้นที่เครื่อง)\n",
        r.root_path.display(),
        fmt::bytes(r.stats.bytes),
        r.stats.files,
        r.stats.dirs,
        fmt::seconds(r.stats.elapsed_ms),
        r.stats.errors,
        fmt::bytes(r.stats.cloud_bytes),
    );
    if top.is_empty() {
        text.push_str("ยังไม่พบรายการที่ควรเก็บกวาดชัดเจน\n");
    } else {
        text.push_str("ที่กินพื้นที่/ข้อมูลที่ไม่ได้ใช้ (5 อันดับแรก):\n");
        for h in hogs.iter().take(5) {
            text.push_str(&format!("- {} [{}] {}\n", h.label, safety_str(h.safety), fmt::bytes(h.size)));
        }
    }
    text.push_str("ขั้นถัดไป: disk_report (รายละเอียดทั้งหมด) และ find_unused (ข้อมูลที่ไม่ได้ใช้)");
    Reply::ok(
        text,
        json!({
            "status": "done",
            "path": r.root_path.to_string_lossy(),
            "totalBytes": r.stats.bytes,
            "total": fmt::bytes(r.stats.bytes),
            "files": r.stats.files,
            "dirs": r.stats.dirs,
            "unreadable": r.stats.errors,
            "cloudOnlyBytes": r.stats.cloud_bytes,
            "cloudOnly": fmt::bytes(r.stats.cloud_bytes),
            "elapsedMs": r.stats.elapsed_ms,
            "topFindings": top,
        }),
    )
}

/* ── Findings ────────────────────────────────────────────────────────────── */

pub fn location_json(l: &Location, now: u64) -> Value {
    let days = fmt::age_days(now, l.last_used.unwrap_or(0));
    json!({
        "path": l.path,
        "bytes": l.size,
        "size": fmt::bytes(l.size),
        "lastUsedDays": days,
        "lastUsed": fmt::age_text(days),
        "note": l.note,
    })
}

pub fn finding_json(h: &Hog, max_locations: usize, now: u64) -> Value {
    json!({
        "id": h.id,
        "label": h.label,
        "safety": safety_str(h.safety),
        "bytes": h.size,
        "size": fmt::bytes(h.size),
        "count": h.count,
        "advice": h.advice,
        "locationsShown": h.locations.len().min(max_locations),
        "locations": h.locations.iter().take(max_locations).map(|l| location_json(l, now)).collect::<Vec<_>>(),
    })
}

fn push_finding_text(text: &mut String, n: usize, h: &Hog, max_locations: usize, now: u64) {
    text.push_str(&format!("\n{n}. {} — {} ({} รายการ) [{}]\n   คำแนะนำ: {}\n", h.label, fmt::bytes(h.size), h.count, safety_label(h.safety), h.advice));
    for l in h.locations.iter().take(max_locations) {
        let age = fmt::age_text(fmt::age_days(now, l.last_used.unwrap_or(0)));
        let note = l.note.as_deref().map(|n| format!(" — {n}")).unwrap_or_default();
        text.push_str(&format!("   - {} ({}, ใช้ล่าสุด {}){}\n", l.path, fmt::bytes(l.size), age, note));
    }
    if h.locations.len() > max_locations {
        text.push_str(&format!("   ... และอีก {} ตำแหน่ง (ดู find_unused / ขอ max_locations เพิ่ม)\n", h.locations.len() - max_locations));
    }
}

pub fn safe_to_reclaim(hogs: &[Hog]) -> u64 {
    hogs.iter().filter(|h| h.safety == Safety::Safe).map(|h| h.size).sum()
}

fn drive_line(r: &ScanResult) -> (Value, String) {
    match report::drive_for(&r.root_path) {
        Some(d) => {
            let j = drive_json(&d);
            let line = format!(
                "ไดรฟ์ {} ว่าง {} จาก {} (ใช้ไป {}%){}",
                d.mount,
                j["free"].as_str().unwrap_or_default(),
                j["total"].as_str().unwrap_or_default(),
                j["usedPercent"],
                if j["nearlyFull"] == true { " — เกือบเต็ม" } else { "" }
            );
            (j, line)
        }
        None => (Value::Null, "ไม่ทราบพื้นที่ว่างของไดรฟ์".to_string()),
    }
}

pub fn report_reply(r: &ScanResult, hogs: &[Hog], max_findings: usize, max_locations: usize, now: u64) -> Reply {
    let (categories, _) = insights::categories(&r.extensions);
    let total_files_bytes: u64 = categories.iter().map(|c| c.bytes).sum();
    let reclaim = safe_to_reclaim(hogs);
    let (drive, drive_text) = drive_line(r);
    let cats: Vec<Value> = categories
        .iter()
        .map(|c| json!({ "category": c.category, "bytes": c.bytes, "size": fmt::bytes(c.bytes), "count": c.count, "percent": fmt::percent(c.bytes, total_files_bytes) }))
        .collect();

    let mut text = format!(
        "รายงานพื้นที่ดิสก์: {}\nสแกน {} ไฟล์ / {} โฟลเดอร์ รวม {}\n{}\nเก็บคืนได้ทันทีแบบปลอดภัย (safe): ประมาณ {}\n\nหมวดไฟล์ที่กินพื้นที่:\n",
        r.root_path.display(),
        r.stats.files,
        r.stats.dirs,
        fmt::bytes(r.stats.bytes),
        drive_text,
        fmt::bytes(reclaim),
    );
    for c in categories.iter().take(8) {
        text.push_str(&format!("- {}: {} ({}%, {} ไฟล์)\n", c.category, fmt::bytes(c.bytes), fmt::percent(c.bytes, total_files_bytes), c.count));
    }
    let shown = hogs.len().min(max_findings);
    text.push_str(&format!("\nสิ่งที่พบ {} รายการ (แสดง {}):", hogs.len(), shown));
    for (i, h) in hogs.iter().take(max_findings).enumerate() {
        push_finding_text(&mut text, i + 1, h, max_locations, now);
    }
    if hogs.is_empty() {
        text.push_str("\n(ไม่พบรายการที่ตรงกับกฎ)\n");
    }

    Reply::ok(
        text.trim_end(),
        json!({
            "root": r.root_path.to_string_lossy(),
            "scan": {
                "totalBytes": r.stats.bytes,
                "total": fmt::bytes(r.stats.bytes),
                "files": r.stats.files,
                "dirs": r.stats.dirs,
                "unreadable": r.stats.errors,
                "cloudOnlyBytes": r.stats.cloud_bytes,
                "elapsedMs": r.stats.elapsed_ms,
            },
            "drive": drive,
            "safeToReclaimBytes": reclaim,
            "safeToReclaim": fmt::bytes(reclaim),
            "categories": cats,
            "findingsTotal": hogs.len(),
            "findings": hogs.iter().take(max_findings).map(|h| finding_json(h, max_locations, now)).collect::<Vec<_>>(),
        }),
    )
}

pub fn unused_reply(r: &ScanResult, hogs: &[Hog], max_locations: usize, now: u64) -> Reply {
    let unused: Vec<&Hog> = hogs.iter().filter(|h| UNUSED_IDS.contains(&h.id)).collect();
    let total: u64 = unused.iter().map(|h| h.size).sum();
    let mut text = format!("ข้อมูลที่อาจไม่ได้ใช้แล้วใน {}\nรวมประมาณ {} (บางส่วนอาจซ้อนกับรายการอื่นใน disk_report ให้ยึด disk_report เป็นหลัก)\n", r.root_path.display(), fmt::bytes(total));
    if unused.is_empty() {
        text.push_str("\nไม่พบไฟล์ใหญ่ที่ไม่ได้แตะเกิน 1 ปี, โปรเจกต์เก่า, ไฟล์ติดตั้งเก่าใน Downloads หรือ AppData ของโปรแกรมที่ถอนไปแล้ว ในขอบเขตที่สแกน\n(รายการเหล่านี้ต้องสแกนโฟลเดอร์ผู้ใช้/ทั้งไดรฟ์ ถ้าสแกนแค่โฟลเดอร์ย่อยอาจไม่ครบ)");
    }
    for (i, h) in unused.iter().enumerate() {
        push_finding_text(&mut text, i + 1, h, max_locations, now);
    }
    Reply::ok(
        text.trim_end(),
        json!({
            "root": r.root_path.to_string_lossy(),
            "totalBytes": total,
            "total": fmt::bytes(total),
            "findings": unused.iter().map(|h| finding_json(h, max_locations, now)).collect::<Vec<_>>(),
        }),
    )
}

/* ── Browse ──────────────────────────────────────────────────────────────── */

pub fn browse_reply(r: &ScanResult, path: Option<&str>, limit: usize, now: u64) -> Reply {
    let target = path.map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).unwrap_or_else(|| r.root_path.to_string_lossy().to_string());
    let Some((ids, file)) = safety::locate(&r.root_path, &r.root, &target) else {
        return Reply::err(format!(
            "ไม่พบ \"{target}\" ในผลสแกน — ต้องอยู่ใต้ {} และไฟล์ที่เล็กกว่า 1 MB ไม่ได้เก็บแยกรายไฟล์ (ดูในรายการ \"ไฟล์ขนาดเล็ก\" ของโฟลเดอร์แม่)",
            r.root_path.display()
        ));
    };
    if let Some(name) = file {
        // A single file: walk to its directory to read size / mtime.
        let mut node = &r.root;
        for &i in &ids {
            node = &node.children[i as usize];
        }
        let Some(f) = node.big_files.iter().find(|f| *f.name == *name) else { return Reply::err("ไม่พบไฟล์ในผลสแกน") };
        let days = fmt::age_days(now, f.modified);
        let category = insights::category_of(&diskwhy_core::scan::extension_of(&f.name));
        return Reply::ok(
            format!("ไฟล์ {target}\nขนาด {} หมวด {category} แก้ไขล่าสุด {}", fmt::bytes(f.size), fmt::age_text(days)),
            json!({ "kind": "file", "path": target, "name": f.name, "bytes": f.size, "size": fmt::bytes(f.size), "category": category, "lastModifiedDays": days, "lastModified": fmt::age_text(days) }),
        );
    }
    let Some(view) = insights::node_view(&r.root, &r.root_path, &ids, limit.max(1)) else { return Reply::err("เปิดโฟลเดอร์ไม่ได้") };
    let mut text = format!("{}  —  {} ใน {} ไฟล์ / {} โฟลเดอร์\n", view.path, fmt::bytes(view.size), view.files, view.dirs);
    let mut children = Vec::new();
    for c in &view.children {
        let child_path = matches!(c.kind, "dir" | "file").then(|| format!("{}\\{}", view.path, c.name));
        let days = fmt::age_days(now, c.newest);
        let pct = fmt::percent(c.size, view.size);
        text.push_str(&format!(
            "- [{}] {} — {} ({}%){}{}\n",
            c.kind,
            c.name,
            fmt::bytes(c.size),
            pct,
            if c.kind == "dir" { format!(", {} ไฟล์", c.files) } else { String::new() },
            if c.newest > 0 { format!(", แก้ไขล่าสุด {}", fmt::age_text(days)) } else { String::new() },
        ));
        children.push(json!({
            "name": c.name,
            "path": child_path,
            "kind": c.kind,
            "bytes": c.size,
            "size": fmt::bytes(c.size),
            "percentOfParent": pct,
            "files": c.files,
            "category": c.category,
            "denied": c.denied,
            "lastModifiedDays": days,
            "lastModified": if c.newest > 0 { fmt::age_text(days) } else { String::new() },
        }));
    }
    Reply::ok(
        text.trim_end(),
        json!({
            "kind": "dir",
            "path": view.path,
            "name": view.name,
            "bytes": view.size,
            "size": fmt::bytes(view.size),
            "files": view.files,
            "dirs": view.dirs,
            "children": children,
        }),
    )
}

/* ── Largest files ───────────────────────────────────────────────────────── */

pub fn largest_reply(r: &ScanResult, limit: usize, min_size_mb: Option<f64>, category: Option<&str>, now: u64) -> Reply {
    if let Some(c) = category {
        if !CATEGORIES.contains(&c) {
            return Reply::err(format!("ไม่รู้จัก category {c:?} — ใช้ได้: {}", CATEGORIES.join(", ")));
        }
    }
    let min_bytes = min_size_mb.map(|mb| (mb.max(0.0) * 1024.0 * 1024.0) as u64).unwrap_or(0);
    let all = insights::top_files(&r.root, &r.root_path, usize::MAX);
    let matching: Vec<_> = all.into_iter().filter(|f| f.size >= min_bytes && category.is_none_or(|c| f.category == c)).collect();
    let shown: Vec<_> = matching.iter().take(limit).collect();
    let mut text = format!(
        "ไฟล์ใหญ่ที่สุด {} จาก {} ไฟล์ที่ตรงเงื่อนไข{}{} (ใน {})\n",
        shown.len(),
        matching.len(),
        category.map(|c| format!(" category={c}")).unwrap_or_default(),
        min_size_mb.map(|m| format!(" ขนาด>={m} MB")).unwrap_or_default(),
        r.root_path.display(),
    );
    let items: Vec<Value> = shown
        .iter()
        .map(|f| {
            let days = fmt::age_days(now, f.modified);
            text.push_str(&format!("- {} ({}, {}, แก้ไขล่าสุด {})\n", f.path, fmt::bytes(f.size), f.category, fmt::age_text(days)));
            json!({ "path": f.path, "name": f.name, "bytes": f.size, "size": fmt::bytes(f.size), "category": f.category, "lastModifiedDays": days, "lastModified": fmt::age_text(days) })
        })
        .collect();
    if shown.is_empty() {
        text.push_str("(ไม่มีไฟล์ที่ตรงเงื่อนไข — ไฟล์ที่เล็กกว่า 1 MB ไม่ได้เก็บรายไฟล์)");
    }
    Reply::ok(text.trim_end(), json!({ "totalMatching": matching.len(), "files": items }))
}

/* ── Admin commands (never executed by DiskWhy) ──────────────────────────── */

/// Facts from the last scan that make the admin command list concrete.
#[derive(Debug, Clone, Default)]
pub struct AdminContext {
    /// Virtual disk files found (path, bytes).
    pub vhdx: Vec<(String, u64)>,
    pub hiberfil_bytes: Option<u64>,
    pub winsxs_bytes: Option<u64>,
}

pub fn admin_context_from(hogs: &[Hog]) -> AdminContext {
    let mut ctx = AdminContext::default();
    for h in hogs {
        match h.id {
            "virtual-disks" => ctx.vhdx = h.locations.iter().map(|l| (l.path.clone(), l.size)).collect(),
            "system-file" => {
                for l in &h.locations {
                    if l.path.to_lowercase().ends_with("hiberfil.sys") {
                        ctx.hiberfil_bytes = Some(l.size);
                    }
                }
            }
            "winsxs" => ctx.winsxs_bytes = Some(h.size),
            _ => {}
        }
    }
    ctx
}

/// Quote a path as a PowerShell single-quoted literal (nothing inside is expanded). None for unsafe characters.
pub fn ps_quote(path: &str) -> Option<String> {
    if path.chars().any(|c| c.is_control()) {
        return None;
    }
    Some(format!("'{}'", path.replace('\'', "''")))
}

pub fn admin_commands_reply(ctx: &AdminContext) -> Reply {
    let mut items: Vec<Value> = Vec::new();

    let dism_frees = match ctx.winsxs_bytes {
        Some(b) => format!("WinSxS ที่พบในผลสแกนมี {} (ขนาดที่เห็นสูงกว่าจริงเพราะ hard link) — มักคืนได้ราว 1-10 GB", fmt::bytes(b)),
        None => "ล้างคอมโพเนนต์เก่าของ Windows Update ที่ถูกแทนที่แล้ว — มักคืนได้ราว 1-10 GB".to_string(),
    };
    items.push(json!({
        "id": "dism-component-cleanup",
        "title": "ล้างคลังคอมโพเนนต์ Windows (WinSxS)",
        "commands": ["DISM /Online /Cleanup-Image /StartComponentCleanup"],
        "frees": dism_frees,
        "tradeOff": "ใช้เวลา 5-30 นาที และถอนการติดตั้งอัปเดตเก่าที่ติดตั้งไปแล้วไม่ได้ (ห้ามใส่ /ResetBase ถ้าไม่จำเป็น เพราะย้อนกลับไม่ได้)",
        "requiresAdmin": true,
    }));

    let hiber_frees = match ctx.hiberfil_bytes {
        Some(b) => format!("ลบ hiberfil.sys ได้ทั้งไฟล์ ประมาณ {}", fmt::bytes(b)),
        None => "ลบ hiberfil.sys (ปกติราว 40-75% ของ RAM)".to_string(),
    };
    items.push(json!({
        "id": "disable-hibernate",
        "title": "ปิด Hibernate",
        "commands": ["powercfg /h off"],
        "frees": hiber_frees,
        "tradeOff": "จะใช้ Hibernate และ Fast Startup ไม่ได้ (เปิดเครื่องช้าลงเล็กน้อย) — เปิดกลับด้วย powercfg /h on",
        "requiresAdmin": true,
    }));

    let mut wsl_commands = Vec::new();
    let mut skipped = 0;
    if !ctx.vhdx.is_empty() {
        wsl_commands.push("wsl --shutdown".to_string());
        for (p, _) in &ctx.vhdx {
            match ps_quote(p) {
                Some(q) => wsl_commands.push(format!("Optimize-VHD -Path {q} -Mode Full")),
                None => skipped += 1,
            }
        }
    }
    let vhdx_total: u64 = ctx.vhdx.iter().map(|(_, b)| b).sum();
    items.push(json!({
        "id": "compact-vhdx",
        "title": "บีบไฟล์ดิสก์เสมือน (WSL / Docker / VM)",
        "commands": wsl_commands,
        "vhdxFound": ctx.vhdx.iter().map(|(p, b)| json!({ "path": p, "bytes": b, "size": fmt::bytes(*b) })).collect::<Vec<_>>(),
        "frees": if ctx.vhdx.is_empty() {
            "ไม่พบไฟล์ .vhdx/.vhd/.vmdk ในผลสแกนล่าสุด จึงไม่มีคำสั่งให้ (สแกนไดรฟ์ทั้งลูกหรือโฟลเดอร์ผู้ใช้เพื่อหาไฟล์เหล่านี้)".to_string()
        } else {
            format!("พบ {} ไฟล์ รวม {} — จะคืนได้เฉพาะพื้นที่ว่างที่อยู่ข้างในดิสก์เสมือน (ข้อมูลข้างในไม่ถูกลบ)", ctx.vhdx.len(), fmt::bytes(vhdx_total))
        },
        "tradeOff": if ctx.vhdx.is_empty() {
            "ไม่มีคำสั่งให้เพราะไม่พบดิสก์เสมือนในผลสแกน".to_string()
        } else {
            "ต้องปิด WSL/Docker/VM ก่อน (wsl --shutdown) และ Optimize-VHD ต้องมี Hyper-V module (Windows Pro ขึ้นไป) — Windows Home ใช้ diskpart: select vdisk file=\"<path>\" › attach vdisk readonly › compact vdisk › detach vdisk; ควรสำรองข้อมูลก่อน (wsl --export)".to_string()
        },
        "unsafePathsSkipped": skipped,
        "requiresAdmin": true,
    }));

    items.push(json!({
        "id": "disk-cleanup-system",
        "title": "Disk Cleanup แบบรวมไฟล์ระบบ",
        "commands": ["cleanmgr /sageset:1", "cleanmgr /sagerun:1"],
        "frees": "เลือกหมวดได้เอง เช่น Windows Update Cleanup, Previous Windows installation(s) (Windows.old), ไฟล์ชั่วคราว, ไฟล์ log — บางเครื่องคืนได้หลาย GB",
        "tradeOff": "ถ้าลบ Windows.old จะย้อนกลับไปเวอร์ชัน Windows เดิมไม่ได้ และถ้าลบ Update Cleanup จะถอนอัปเดตเก่าไม่ได้ — sageset:1 จะเปิดหน้าต่างให้ติ๊กเลือกก่อน แล้ว sagerun:1 จึงลงมือทำ",
        "requiresAdmin": true,
    }));

    let mut text = String::from(
        "คำสั่งที่ต้องรันด้วยสิทธิ์ผู้ดูแลระบบ — DiskWhy จะไม่รันให้ ผู้ใช้ต้องรันเอง:\nวิธีเปิด: คลิกขวาที่ปุ่ม Start › Terminal (Admin) แล้ววางทีละคำสั่ง\n",
    );
    for (i, it) in items.iter().enumerate() {
        text.push_str(&format!("\n{}. {}\n", i + 1, it["title"].as_str().unwrap_or_default()));
        if let Some(cmds) = it["commands"].as_array() {
            for c in cmds {
                text.push_str(&format!("   > {}\n", c.as_str().unwrap_or_default()));
            }
        }
        text.push_str(&format!("   คืนพื้นที่: {}\n   ข้อแลกเปลี่ยน: {}\n", it["frees"].as_str().unwrap_or_default(), it["tradeOff"].as_str().unwrap_or_default()));
    }
    Reply::ok(
        text.trim_end(),
        json!({
            "howToOpen": "Right-click Start > Terminal (Admin), paste one command at a time",
            "executedByDiskWhy": false,
            "items": items,
        }),
    )
}

/// Does `child` lie inside (or equal) `parent`? Case-insensitive, separator-agnostic.
pub fn path_is_within(parent: &Path, child: &str) -> bool {
    let norm = |s: &str| s.replace('/', "\\").trim_end_matches('\\').to_lowercase();
    let (p, c) = (norm(&parent.to_string_lossy()), norm(child));
    c == p || (c.starts_with(&p) && c.as_bytes().get(p.len()) == Some(&b'\\'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use diskwhy_core::report::run_scan;
    use diskwhy_core::scan::Progress;
    use std::sync::atomic::AtomicBool;

    fn scanned() -> (tempfile::TempDir, ScanResult) {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        std::fs::create_dir_all(r.join("proj/node_modules/lib")).unwrap();
        std::fs::write(r.join("proj/package.json"), b"{}").unwrap();
        std::fs::write(r.join("proj/node_modules/lib/big.bin"), vec![1u8; 3 * 1024 * 1024]).unwrap();
        std::fs::write(r.join("movie.mp4"), vec![2u8; 2 * 1024 * 1024]).unwrap();
        std::fs::write(r.join("notes.txt"), b"hello").unwrap();
        let res = run_scan(r, &Progress::default(), &AtomicBool::new(false)).unwrap();
        (dir, res)
    }

    #[test]
    fn drives_view_flags_nearly_full() {
        let mk = |used: u64| DriveInfo { mount: "C:\\".into(), name: "Win".into(), file_system: "NTFS".into(), total: 1000 * 1024 * 1024, available: (1000 - used) * 1024 * 1024, removable: false };
        let r = drives_reply_from(&[mk(950), mk(500)]);
        assert!(!r.is_error);
        assert_eq!(r.data["drives"][0]["nearlyFull"], true);
        assert_eq!(r.data["drives"][1]["nearlyFull"], false);
        assert_eq!(r.data["drives"][0]["usedPercent"], 95.0);
        assert!(r.text.contains("เกือบเต็ม"));
        assert!(drives_reply_from(&[]).is_error);
    }

    #[test]
    fn report_has_categories_findings_and_safe_total() {
        let (_d, res) = scanned();
        let hogs = insights::find_hogs_with(&res.root, &res.root_path, &[], fmt::now_secs() + 400 * 86_400);
        let rep = report_reply(&res, &hogs, 20, 8, fmt::now_secs() + 400 * 86_400);
        assert!(!rep.is_error);
        let cats = rep.data["categories"].as_array().unwrap();
        assert!(cats.iter().any(|c| c["category"] == "video"));
        let findings = rep.data["findings"].as_array().unwrap();
        assert!(findings.iter().any(|f| f["id"] == "node-modules" || f["id"] == "stale-projects"));
        for f in findings {
            assert!(["safe", "review", "system"].contains(&f["safety"].as_str().unwrap()));
            assert!(f["locations"].as_array().unwrap().len() <= 8);
            assert!(f["size"].is_string() && f["bytes"].is_u64());
        }
        assert!(rep.data["safeToReclaimBytes"].as_u64().unwrap() >= 3 * 1024 * 1024);
        assert!(rep.text.contains("รายงานพื้นที่ดิสก์"));
        // limits are honoured
        let small = report_reply(&res, &hogs, 1, 1, 0);
        assert_eq!(small.data["findings"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn unused_view_only_has_stale_findings() {
        let (_d, res) = scanned();
        let now = fmt::now_secs() + 400 * 86_400; // pretend a year has passed
        let hogs = insights::find_hogs_with(&res.root, &res.root_path, &[], now);
        let u = unused_reply(&res, &hogs, 30, now);
        let ids: Vec<_> = u.data["findings"].as_array().unwrap().iter().map(|f| f["id"].as_str().unwrap().to_string()).collect();
        assert!(ids.contains(&"stale-projects".to_string()), "{ids:?}");
        assert!(ids.iter().all(|i| UNUSED_IDS.contains(&i.as_str())));
        let loc = &u.data["findings"][0]["locations"][0];
        assert!(loc["lastUsedDays"].as_u64().unwrap() >= 90 && loc["note"].is_string());
    }

    #[test]
    fn browse_lists_children_sorted_with_percent() {
        let (d, res) = scanned();
        let root = browse_reply(&res, None, 50, fmt::now_secs());
        assert!(!root.is_error, "{}", root.text);
        let kids = root.data["children"].as_array().unwrap();
        assert_eq!(kids[0]["name"], "proj", "largest first");
        let pct: f64 = kids.iter().map(|k| k["percentOfParent"].as_f64().unwrap()).sum();
        assert!((pct - 100.0).abs() < 0.5, "{pct}");
        // drill down by absolute path
        let sub = browse_reply(&res, Some(&d.path().join("proj").to_string_lossy()), 50, fmt::now_secs());
        assert_eq!(sub.data["children"][0]["name"], "node_modules");
        // a file
        let f = browse_reply(&res, Some(&d.path().join("movie.mp4").to_string_lossy()), 50, fmt::now_secs());
        assert_eq!(f.data["kind"], "file");
        assert_eq!(f.data["category"], "video");
        // outside the scan
        assert!(browse_reply(&res, Some("Z:\\nope"), 50, 0).is_error);
    }

    #[test]
    fn largest_files_filters() {
        let (_d, res) = scanned();
        let all = largest_reply(&res, 30, None, None, 0);
        assert_eq!(all.data["totalMatching"], 2);
        assert_eq!(all.data["files"][0]["name"], "big.bin");
        let big = largest_reply(&res, 30, Some(2.5), None, 0);
        assert_eq!(big.data["files"].as_array().unwrap().len(), 1);
        let vid = largest_reply(&res, 30, None, Some("video"), 0);
        assert_eq!(vid.data["files"][0]["name"], "movie.mp4");
        assert_eq!(largest_reply(&res, 1, None, None, 0).data["files"].as_array().unwrap().len(), 1);
        assert!(largest_reply(&res, 30, None, Some("nonsense"), 0).is_error);
    }

    #[test]
    fn admin_commands_content() {
        let ctx = AdminContext {
            vhdx: vec![("C:\\Users\\a\\AppData\\Local\\Docker\\wsl\\data\\ext4.vhdx".into(), 5 << 30), ("D:\\it's\\disk.vhdx".into(), 1 << 30)],
            hiberfil_bytes: Some(6 << 30),
            winsxs_bytes: None,
        };
        let r = admin_commands_reply(&ctx);
        assert_eq!(r.data["executedByDiskWhy"], false);
        let all = serde_json::to_string(&r.data).unwrap();
        for needle in ["DISM /Online /Cleanup-Image /StartComponentCleanup", "powercfg /h off", "wsl --shutdown", "cleanmgr /sageset:1", "cleanmgr /sagerun:1"] {
            assert!(all.contains(needle), "missing {needle}");
        }
        assert!(all.contains("Optimize-VHD -Path 'C:\\\\Users\\\\a\\\\AppData\\\\Local\\\\Docker\\\\wsl\\\\data\\\\ext4.vhdx' -Mode Full"), "{all}");
        assert!(all.contains("it''s"), "single quotes are doubled");
        assert!(r.text.contains("Terminal (Admin)"));
        // no VHDX found -> no Optimize-VHD at all
        let none = admin_commands_reply(&AdminContext::default());
        assert!(!serde_json::to_string(&none.data).unwrap().contains("Optimize-VHD"));
        assert!(ps_quote("bad\npath").is_none());
    }

    #[test]
    fn within_is_case_and_separator_insensitive() {
        let root = Path::new("C:\\Users\\a");
        assert!(path_is_within(root, "c:/users/A/x"));
        assert!(path_is_within(root, "C:\\Users\\a"));
        assert!(!path_is_within(root, "C:\\Users\\ab\\x"));
        assert!(!path_is_within(root, "D:\\Users\\a"));
    }
}
