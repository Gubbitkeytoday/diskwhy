//! Turns a scanned tree into answers: what kinds of files, which known space hogs, which giant files.

use crate::scan::DirNode;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

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

pub fn find_hogs(root: &DirNode, root_path: &Path) -> Vec<Hog> {
    let mut hits: HashMap<&'static str, Hog> = HashMap::new();
    let base = root_path.to_string_lossy().trim_end_matches(['\\', '/']).to_string();

    fn visit(node: &DirNode, path: &str, ids: &mut Vec<u32>, hits: &mut HashMap<&'static str, Hog>) {
        let lower = path.to_lowercase().replace('/', "\\");
        let name = node.name.to_lowercase();
        if let Some(rule) = RULES.iter().find(|r| (r.test)(&lower, &name)) {
            let hog = hits.entry(rule.id).or_insert_with(|| Hog {
                id: rule.id,
                label: rule.label,
                advice: rule.advice,
                safety: rule.safety,
                size: 0,
                count: 0,
                locations: vec![],
            });
            hog.size += node.size;
            hog.count += 1;
            hog.locations.push(Location { path: path.to_string(), size: node.size, ids: ids.clone() });
            return; // don't double count nested matches (e.g. node_modules inside node_modules)
        }
        for (i, child) in node.children.iter().enumerate() {
            ids.push(i as u32);
            visit(child, &format!("{path}\\{}", child.name), ids, hits);
            ids.pop();
        }
    }

    let mut ids = Vec::new();
    for (i, child) in root.children.iter().enumerate() {
        ids.push(i as u32);
        visit(child, &format!("{base}\\{}", child.name), &mut ids, &mut hits);
        ids.pop();
    }

    // Root-level system files and virtual disks anywhere in big files.
    for f in &root.big_files {
        let lname = f.name.to_lowercase();
        if let Some((_, label, advice)) = SYSTEM_FILES.iter().find(|(n, _, _)| *n == lname) {
            hits.insert(
                label,
                Hog {
                    id: "system-file",
                    label,
                    advice,
                    safety: Safety::System,
                    size: f.size,
                    count: 1,
                    locations: vec![Location { path: format!("{base}\\{}", f.name), size: f.size, ids: vec![] }],
                },
            );
        }
    }
    collect_vhdx(root, &base, &mut Vec::new(), &mut hits);

    let mut out: Vec<Hog> = hits.into_values().filter(|h| h.size > 0).collect();
    for h in &mut out {
        h.locations.sort_unstable_by(|a, b| b.size.cmp(&a.size));
        h.locations.truncate(20);
    }
    out.sort_unstable_by(|a, b| b.size.cmp(&a.size));
    out
}

fn collect_vhdx(node: &DirNode, path: &str, ids: &mut Vec<u32>, hits: &mut HashMap<&'static str, Hog>) {
    for f in &node.big_files {
        let l = f.name.to_lowercase();
        if l.ends_with(".vhdx") || l.ends_with(".vhd") || l.ends_with(".vmdk") {
            let hog = hits.entry("virtual-disks").or_insert_with(|| {
                let r = RULES.iter().find(|r| r.id == "virtual-disks").expect("rule exists");
                Hog { id: r.id, label: r.label, advice: r.advice, safety: r.safety, size: 0, count: 0, locations: vec![] }
            });
            hog.size += f.size;
            hog.count += 1;
            hog.locations.push(Location { path: format!("{path}\\{}", f.name), size: f.size, ids: ids.clone() });
        }
    }
    for (i, c) in node.children.iter().enumerate() {
        ids.push(i as u32);
        collect_vhdx(c, &format!("{path}\\{}", c.name), ids, hits);
        ids.pop();
    }
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
}

pub fn top_files(root: &DirNode, root_path: &Path, limit: usize) -> Vec<TopFile> {
    let base = root_path.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
    let mut all: Vec<(u64, String, String, Vec<u32>)> = Vec::new();
    fn visit(node: &DirNode, path: &str, ids: &mut Vec<u32>, out: &mut Vec<(u64, String, String, Vec<u32>)>) {
        for f in &node.big_files {
            out.push((f.size, format!("{path}\\{}", f.name), f.name.to_string(), ids.clone()));
        }
        for (i, c) in node.children.iter().enumerate() {
            ids.push(i as u32);
            visit(c, &format!("{path}\\{}", c.name), ids, out);
            ids.pop();
        }
    }
    visit(root, &base, &mut Vec::new(), &mut all);
    all.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    all.truncate(limit);
    all.into_iter()
        .map(|(size, path, name, dir_ids)| {
            let category = category_of(&crate::scan::extension_of(&name));
            TopFile { path, name, size, category, dir_ids }
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
        })
        .chain(node.big_files.iter().map(|f| ChildView {
            kind: "file",
            name: f.name.to_string(),
            size: f.size,
            files: 1,
            index: None,
            category: Some(category_of(&crate::scan::extension_of(&f.name))),
            denied: false,
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
    use crate::scan::{DirNode, FileEntry};

    fn dir(name: &str, size: u64, children: Vec<DirNode>) -> DirNode {
        DirNode { name: name.into(), size, children, ..Default::default() }
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
        root.big_files = vec![FileEntry { name: "hiberfil.sys".into(), size: 350 }];
        root
    }

    #[test]
    fn detects_hogs_without_double_counting() {
        let hogs = find_hogs(&sample(), Path::new("C:\\"));
        let get = |id: &str| hogs.iter().find(|h| h.id == id).map(|h| (h.size, h.count));
        assert_eq!(get("downloads"), Some((300, 1)));
        assert_eq!(get("temp"), Some((150, 1)));
        assert_eq!(get("dev-caches"), Some((50, 1)));
        assert_eq!(get("node-modules"), Some((90, 1)), "nested node_modules must not be counted twice");
        assert_eq!(get("recycle-bin"), Some((50, 1)));
        assert_eq!(get("system-file"), Some((350, 1)));
        let temp = hogs.iter().find(|h| h.id == "temp").unwrap();
        assert_eq!(temp.locations[0].path, "C:\\Users\\somchai\\AppData\\Local\\Temp");
        assert_eq!(temp.locations[0].ids, vec![0, 0, 1, 0, 0]);
        assert_eq!(hogs[0].size, 350, "sorted largest first");
    }

    #[test]
    fn node_view_paths_and_limits() {
        let root = sample();
        let v = node_view(&root, Path::new("C:\\"), &[0, 0], 2).unwrap();
        assert_eq!(v.path, "C:\\Users\\somchai");
        assert_eq!(v.crumbs.len(), 3);
        assert_eq!(v.children.len(), 3, "2 items + 'rest'");
        assert_eq!(v.children[2].kind, "rest");
        assert_eq!(v.children[2].size, 100);
        assert!(node_view(&root, Path::new("C:\\"), &[9], 10).is_none());
    }

    #[test]
    fn categorises_extensions() {
        assert_eq!(category_of("mkv"), "video");
        assert_eq!(category_of("vhdx"), "disk-image");
        assert_eq!(category_of("msi"), "installer");
        assert_eq!(category_of("weird"), "other");
    }
}
