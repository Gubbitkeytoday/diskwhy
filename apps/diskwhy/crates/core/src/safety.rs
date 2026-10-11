//! Safety rules shared by every surface (desktop app, MCP server, CLI). Nothing deletes without passing these.

use crate::scan::DirNode;
use std::path::{Path, PathBuf};

/// Paths DiskWhy refuses to touch, even via the Recycle Bin.
pub fn is_protected(path: &Path) -> bool {
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

pub const PROTECTED_MESSAGE: &str = "ตำแหน่งนี้เป็นไฟล์/โฟลเดอร์ของระบบ — DiskWhy ไม่อนุญาตให้ลบเพื่อความปลอดภัย";

/// Does `dir_ids` (+ optional file name) resolve to `target` in the scanned tree? Case-insensitive, separator-agnostic.
pub fn tree_path_matches(root_path: &Path, root: &DirNode, dir_ids: &[u32], file_name: Option<&str>, target: &str) -> bool {
    let mut built = norm(&root_path.to_string_lossy());
    let mut node = root;
    for &i in dir_ids {
        let Some(child) = node.children.get(i as usize) else { return false };
        built.push('\\');
        built.push_str(&child.name.to_lowercase());
        node = child;
    }
    if let Some(f) = file_name {
        if !node.big_files.iter().any(|x| &*x.name == f) {
            return false;
        }
        built.push('\\');
        built.push_str(&f.to_lowercase());
    }
    built == norm(target)
}

/// Resolve an absolute path to its location in the scanned tree: `(dir_ids, file_name)`.
/// Returns None if the path is outside the scan root or not present in the tree (e.g. a small file below 1 MB).
pub fn locate(root_path: &Path, root: &DirNode, target: &str) -> Option<(Vec<u32>, Option<String>)> {
    let base = norm(&root_path.to_string_lossy());
    let t = norm(target);
    let rest = t.strip_prefix(&base)?;
    if !(rest.is_empty() || rest.starts_with('\\')) {
        return None;
    }
    let segs: Vec<&str> = rest.split('\\').filter(|s| !s.is_empty()).collect();
    let mut ids = Vec::with_capacity(segs.len());
    let mut node = root;
    for (i, seg) in segs.iter().enumerate() {
        if let Some(idx) = node.children.iter().position(|c| c.name.to_lowercase() == *seg) {
            ids.push(idx as u32);
            node = &node.children[idx];
        } else if i == segs.len() - 1 {
            let f = node.big_files.iter().find(|f| f.name.to_lowercase() == *seg)?;
            return Some((ids, Some(f.name.to_string())));
        } else {
            return None;
        }
    }
    Some((ids, None))
}

/// Move a path to the Recycle Bin (recoverable) after the protection check. Never hard-deletes.
pub fn recycle(path: &Path) -> Result<(), String> {
    if is_protected(path) {
        return Err(PROTECTED_MESSAGE.into());
    }
    if !path.exists() {
        return Err(format!("ไม่พบไฟล์หรือโฟลเดอร์: {}", path.display()));
    }
    trash::delete(path).map_err(|e| format!("ย้ายไปถังขยะไม่สำเร็จ: {e}"))
}

fn norm(s: &str) -> String {
    s.replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// Expand `%VAR%` style environment variables in a Windows path (used for known cache locations).
pub fn expand_env(path: &str) -> PathBuf {
    let mut out = String::new();
    let mut rest = path;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('%') {
            let var = &after[..end];
            out.push_str(&std::env::var(var).unwrap_or_default());
            rest = &after[end + 1..];
        } else {
            out.push_str(&rest[start..]);
            rest = "";
        }
    }
    out.push_str(rest);
    PathBuf::from(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::FileEntry;

    #[test]
    fn protects_system_locations() {
        for p in ["C:\\", "C:\\Windows", "c:\\windows\\System32\\x.dll", "C:\\Program Files\\App", "C:\\Users", "D:\\", "C:\\pagefile.sys", "E:\\$Recycle.Bin"] {
            assert!(is_protected(Path::new(p)), "{p} should be protected");
        }
        for p in ["C:\\Users\\somchai\\Downloads\\setup.exe", "D:\\Videos\\old.mkv", "C:\\Users\\a\\code\\app\\node_modules"] {
            assert!(!is_protected(Path::new(p)), "{p} should be allowed");
        }
    }

    fn sample() -> DirNode {
        let leaf = DirNode { name: "node_modules".into(), ..Default::default() };
        let mut app = DirNode { name: "app".into(), children: vec![leaf], ..Default::default() };
        app.big_files = vec![FileEntry { name: "setup.exe".into(), size: 5, modified: 0 }];
        DirNode { name: "C:\\".into(), children: vec![app], ..Default::default() }
    }

    #[test]
    fn trash_target_must_match_tree_node() {
        let root = sample();
        let rp = Path::new("C:\\Users\\a\\");
        assert!(tree_path_matches(rp, &root, &[0, 0], None, "c:/users/A/app/node_modules/"));
        assert!(tree_path_matches(rp, &root, &[0], Some("setup.exe"), "C:\\Users\\a\\app\\setup.exe"));
        assert!(!tree_path_matches(rp, &root, &[0], None, "C:\\Users\\a\\app\\node_modules"));
        assert!(!tree_path_matches(rp, &root, &[1], None, "C:\\Users\\a\\other"));
        assert!(!tree_path_matches(rp, &root, &[0], Some("gone.exe"), "C:\\Users\\a\\app\\gone.exe"));
    }

    #[test]
    fn locates_paths_in_tree() {
        let root = sample();
        let rp = Path::new("C:\\Users\\a");
        assert_eq!(locate(rp, &root, "c:\\users\\a\\APP\\node_modules"), Some((vec![0, 0], None)));
        assert_eq!(locate(rp, &root, "C:\\Users\\a\\app\\setup.exe"), Some((vec![0], Some("setup.exe".into()))));
        assert_eq!(locate(rp, &root, "C:\\Users\\a"), Some((vec![], None)));
        assert_eq!(locate(rp, &root, "C:\\Users\\ab\\app"), None, "prefix of another folder name");
        assert_eq!(locate(rp, &root, "D:\\x"), None);
        assert_eq!(locate(rp, &root, "C:\\Users\\a\\app\\tiny.txt"), None);
    }

    #[test]
    fn expands_env_vars() {
        // SAFETY: test-only env mutation, single-threaded within this test.
        unsafe { std::env::set_var("DISKWHY_TEST_DIR", "C:\\x") };
        assert_eq!(expand_env("%DISKWHY_TEST_DIR%\\cache"), PathBuf::from("C:\\x\\cache"));
        assert_eq!(expand_env("no vars"), PathBuf::from("no vars"));
    }
}
