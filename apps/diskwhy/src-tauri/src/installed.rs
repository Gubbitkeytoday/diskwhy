//! Which programs are installed? Used to tell live AppData folders from orphans left behind by uninstalled apps.
//!
//! * On Windows the list comes from the real "Uninstall" registry keys (HKLM, HKLM\WOW6432Node, HKCU:
//!   `DisplayName`, `Publisher`, `InstallLocation`) plus the top-level folders of `Program Files` / `Program Files (x86)`.
//! * Everything is reduced to lowercase ASCII-alphanumeric "tokens" (whole names and individual words).
//! * Read errors are ignored on purpose: missing keys and unreadable values are normal on real machines.
//! * Matching is deliberately generous: a false "known" only hides a suggestion, a false "orphan" could cost the user data.

use std::collections::BTreeSet;
use std::sync::OnceLock;

/// Folders that belong to Windows, shared vendor runtimes or dev-tool caches: never reported as orphans.
/// Stored normalized (lowercase ASCII alphanumerics only) so they compare directly with `normalize` output.
const ALWAYS_KNOWN: &[&str] = &[
    // Windows / system
    "microsoft",
    "packages",
    "temp",
    "tmp",
    "programs",
    "comms",
    "connecteddevicesplatform",
    "d3dscache",
    "crashdumps",
    "publishers",
    "packagemanagement",
    "packagecache",
    "isolatedstorage",
    "squirreltemp",
    "fontconfig",
    "history",
    "virtualstore",
    "ssh",
    "tempstate",
    "elevateddiagnostics",
    "placeholdertilelogofolder",
    "systemdata",
    "usoprivate",
    "usoshared",
    "cache",
    "caches",
    // GPU / chipset / shared vendor folders
    "nvidia",
    "nvidiacorporation",
    "intel",
    "amd",
    "apple",
    "appleinc",
    "adobe",
    "windows",
    // Browsers / popular launchers and tools that share folders between products
    "mozilla",
    "google",
    "bravesoftware",
    "gitkraken",
    "chocolatey",
    // Developer caches (regenerable, handled by their own rules, not "program data")
    "nodegyp",
    "electron",
    "electronbuilder",
    "gobuild",
    "pip",
    "pipenv",
    "npm",
    "npmcache",
    "pnpm",
    "yarn",
    "typescript",
];

/// Vendor prefixes: "MicrosoftEdge", "WindowsApps", "NVIDIA Corporation", "regid.1991-06.com.microsoft" …
const ALWAYS_KNOWN_PREFIXES: &[&str] = &["microsoft", "windows", "nvidia", "regid"];

/// Words too generic to identify a program (they would "match" half of AppData).
const STOP_WORDS: &[&str] = &[
    "inc", "ltd", "llc", "corp", "corporation", "company", "limited", "the", "and", "for", "with", "software", "setup", "installer",
    "update", "updater", "runtime", "redistributable", "driver", "drivers", "edition", "version", "tool", "tools", "service",
    "services", "pack", "package", "program", "programs", "files", "application", "applications", "app", "apps", "x64", "x86",
    "win32", "win64", "english", "international",
];

/// Lowercase ASCII alphanumerics only ("Visual Studio Code" → "visualstudiocode"). Non-ASCII letters are dropped.
pub fn normalize(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// Add the whole normalized name and each meaningful word of `name` to `out`.
/// Words of fewer than 3 characters, pure numbers (versions) and generic stop-words are skipped.
#[cfg_attr(not(windows), allow(dead_code))]
fn add_name_tokens(name: &str, out: &mut BTreeSet<String>) {
    let whole = normalize(name);
    if whole.len() >= 2 {
        out.insert(whole);
    }
    for word in name.split(|c: char| !c.is_ascii_alphanumeric()) {
        let w = word.to_ascii_lowercase();
        if w.len() >= 3 && !w.bytes().all(|b| b.is_ascii_digit()) && !STOP_WORDS.contains(&w.as_str()) {
            out.insert(w);
        }
    }
}

/// Last non-empty segment of a (possibly quoted, possibly trailing-slashed) path.
#[cfg_attr(not(windows), allow(dead_code))]
fn last_path_segment(path: &str) -> Option<&str> {
    path.trim().trim_matches('"').rsplit(['\\', '/']).find(|s| !s.is_empty())
}

/// Is `folder` (an AppData / ProgramData folder name) explained by an installed program or a Windows/vendor allowlist entry?
///
/// `tokens` must be normalized (as produced by [`installed_program_tokens`]). A folder is known when
/// * its normalized name is allowlisted (exactly, or by vendor prefix), or
/// * it equals a token, or
/// * it (4+ characters) is contained in a token, or a token (4+ characters) is contained in it.
///
/// A name with no ASCII letters or digits (e.g. Thai only) can't be judged and counts as known.
pub fn folder_is_known(folder: &str, tokens: &[String]) -> bool {
    let f = normalize(folder);
    if f.is_empty() || ALWAYS_KNOWN.contains(&f.as_str()) || ALWAYS_KNOWN_PREFIXES.iter().any(|p| f.starts_with(p)) {
        return true;
    }
    tokens.iter().any(|t| *t == f || (f.len() >= 4 && t.contains(f.as_str())) || (t.len() >= 4 && f.contains(t.as_str())))
}

/// Normalized tokens of every installed program, computed once per process (registry reads are slow-ish).
/// Empty on non-Windows platforms or if nothing could be read.
pub fn installed_program_tokens() -> &'static [String] {
    static TOKENS: OnceLock<Vec<String>> = OnceLock::new();
    TOKENS.get_or_init(|| {
        let mut set = BTreeSet::new();
        collect_platform_tokens(&mut set);
        set.into_iter().collect()
    })
}

#[cfg(not(windows))]
fn collect_platform_tokens(_out: &mut BTreeSet<String>) {}

#[cfg(windows)]
fn collect_platform_tokens(out: &mut BTreeSet<String>) {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};

    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    const UNINSTALL_32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

    fn read_uninstall(hive: &RegKey, path: &str, out: &mut BTreeSet<String>) {
        let Ok(base) = hive.open_subkey_with_flags(path, KEY_READ) else { return };
        for sub in base.enum_keys().flatten() {
            // Inno Setup names its keys "<AppName>_is1"; GUID keys carry no information.
            if !sub.starts_with('{') {
                add_name_tokens(sub.strip_suffix("_is1").unwrap_or(&sub), out);
            }
            let Ok(key) = base.open_subkey_with_flags(&sub, KEY_READ) else { continue };
            for value in ["DisplayName", "Publisher"] {
                if let Ok(v) = key.get_value::<String, _>(value) {
                    add_name_tokens(&v, out);
                }
            }
            if let Ok(loc) = key.get_value::<String, _>("InstallLocation")
                && let Some(seg) = last_path_segment(&loc)
            {
                add_name_tokens(seg, out);
            }
        }
    }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    read_uninstall(&hklm, UNINSTALL, out);
    read_uninstall(&hklm, UNINSTALL_32, out);
    read_uninstall(&RegKey::predef(HKEY_CURRENT_USER), UNINSTALL, out);

    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        let dir = std::env::var_os(var).unwrap_or_else(|| if var == "ProgramFiles" { r"C:\Program Files".into() } else { r"C:\Program Files (x86)".into() });
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for entry in rd.flatten() {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                add_name_tokens(&entry.file_name().to_string_lossy(), out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(names: &[&str]) -> Vec<String> {
        let mut set = BTreeSet::new();
        for n in names {
            add_name_tokens(n, &mut set);
        }
        set.into_iter().collect()
    }

    #[test]
    fn normalizes() {
        assert_eq!(normalize("Visual Studio Code"), "visualstudiocode");
        assert_eq!(normalize("7-Zip"), "7zip");
        assert_eq!(normalize("โปรแกรม"), "");
        assert_eq!(normalize("A_b.C"), "abc");
    }

    #[test]
    fn tokenises_names_into_whole_and_words() {
        let t = tokens(&["Microsoft Visual Studio Code (x64) 1.95"]);
        assert!(t.contains(&"microsoft".to_string()));
        assert!(t.contains(&"visual".to_string()));
        assert!(t.contains(&"code".to_string()));
        assert!(!t.contains(&"x64".to_string()), "stop-word");
        assert!(!t.contains(&"1".to_string()) && !t.contains(&"95".to_string()), "version numbers are not tokens");
        assert!(t.iter().any(|x| x.starts_with("microsoftvisualstudiocode")), "whole name is a token");
    }

    #[test]
    fn matches_installed_programs() {
        let t = tokens(&["Spotify", "Discord Inc.", "Adobe Photoshop 2024", "7-Zip 23.01 (x64)", "Mozilla Firefox"]);
        assert!(folder_is_known("Spotify", &t), "exact");
        assert!(folder_is_known("spotify", &t), "case-insensitive");
        assert!(folder_is_known("discord", &t));
        assert!(folder_is_known("7-Zip", &t), "punctuation ignored");
        assert!(folder_is_known("Photoshop", &t), "folder is a word of the program name");
        assert!(folder_is_known("DiscordPTB", &t), "token (4+) contained in the folder name");
        assert!(folder_is_known("Adobe Photoshop 2024 Cache", &t), "whole name contained in the folder name");
    }

    #[test]
    fn unmatched_folders_are_orphans() {
        let t = tokens(&["Spotify", "Discord Inc."]);
        assert!(!folder_is_known("OldGameLauncher", &t));
        assert!(!folder_is_known("{3F2504E0-4F89-11D3-9A0C-0305E82C3301}", &t));
        assert!(!folder_is_known("abc", &t), "short folder names must match exactly, not by containment");
        assert!(!folder_is_known("OldTool", &[]), "nothing installed, nothing known");
    }

    #[test]
    fn short_names_only_match_exactly() {
        let t = tokens(&["Git", "VLC media player"]);
        assert!(folder_is_known("git", &t));
        assert!(folder_is_known("VLC", &t));
        assert!(!folder_is_known("gi", &t));
        assert!(!folder_is_known("vl", &t));
    }

    #[test]
    fn allowlist_wins_without_any_installed_token() {
        for name in [
            "Microsoft", "Packages", "Temp", "Programs", "Comms", "ConnectedDevicesPlatform", "D3DSCache", "CrashDumps", "Publishers",
            "PackageManagement", "Package Cache", "NVIDIA", "NVIDIA Corporation", "Intel", "AMD", "Apple", "Adobe", "Windows",
            "IsolatedStorage", "SquirrelTemp", "fontconfig", "History", "VirtualStore", "Mozilla", "Google", "BraveSoftware", "Ssh",
            "GitKraken", "MicrosoftEdge", "WindowsApps", "regid.1991-06.com.microsoft", "node-gyp",
        ] {
            assert!(folder_is_known(name, &[]), "{name} must be allowlisted");
        }
    }

    #[test]
    fn unjudgeable_names_count_as_known() {
        assert!(folder_is_known("ข้อมูลแอป", &[]));
        assert!(folder_is_known("", &[]));
    }

    #[test]
    fn path_segments() {
        assert_eq!(last_path_segment(r"C:\Program Files\Foo Bar\"), Some("Foo Bar"));
        assert_eq!(last_path_segment(r#""D:\Games\Quake""#), Some("Quake"));
        assert_eq!(last_path_segment(""), None);
        assert_eq!(last_path_segment("\\\\"), None);
    }

    #[test]
    fn real_registry_read_does_not_panic() {
        // Pure smoke test of the real machine state: whatever is installed, the call must succeed and be stable.
        let a = installed_program_tokens();
        let b = installed_program_tokens();
        assert!(std::ptr::eq(a, b), "cached");
        if cfg!(windows) {
            assert!(!a.is_empty(), "a Windows machine always has installed programs registered");
        }
        assert!(a.iter().all(|t| t == &normalize(t) || t.chars().all(|c| c.is_ascii_alphanumeric())));
    }
}
