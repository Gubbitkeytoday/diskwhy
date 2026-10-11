//! Smoke tests for the command line interface (the real binary).

use serde_json::Value;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_diskwhy-cli"))
}

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("app/node_modules")).unwrap();
    std::fs::write(dir.path().join("app/package.json"), b"{}").unwrap();
    std::fs::write(dir.path().join("app/node_modules/big.bin"), vec![1u8; 3 * 1024 * 1024]).unwrap();
    std::fs::write(dir.path().join("movie.mp4"), vec![2u8; 2 * 1024 * 1024]).unwrap();
    dir
}

fn stdout_of(args: &[&str]) -> String {
    let out = bin().args(args).output().unwrap();
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn version_and_help() {
    let v = stdout_of(&["--version"]);
    assert_eq!(v.trim(), format!("diskwhy-cli {}", env!("CARGO_PKG_VERSION")));
    let help = stdout_of(&["--help"]);
    for needle in ["drives", "report", "unused", "top", "cleanup", "mcp", "ทำไมดิสก์เต็ม", "Explain why"] {
        assert!(help.contains(needle), "--help lacks {needle}");
    }
    assert!(stdout_of(&["report", "--help"]).contains("--top"));
}

#[test]
fn report_json_parses() {
    let dir = fixture();
    let out = stdout_of(&["report", &dir.path().to_string_lossy(), "--json", "--top", "5"]);
    let v: Value = serde_json::from_str(&out).expect("report --json is valid JSON");
    assert_eq!(v["scan"]["files"], 3);
    assert_eq!(v["scan"]["totalBytes"], 5 * 1024 * 1024 + 2);
    assert!(v["findings"].as_array().unwrap().len() <= 5);
    assert!(v["categories"].as_array().unwrap().iter().any(|c| c["category"] == "video"));
    assert_eq!(v["topFiles"][0]["name"], "big.bin");
    assert!(v["safeToReclaimBytes"].as_u64().unwrap() >= 3 * 1024 * 1024);
}

#[test]
fn report_human_output_is_thai() {
    let dir = fixture();
    let out = stdout_of(&["report", &dir.path().to_string_lossy(), "--top", "3"]);
    assert!(out.contains("รายงานพื้นที่ดิสก์") && out.contains("MB"), "{out}");
}

#[test]
fn unused_and_top_json() {
    let dir = fixture();
    let p = dir.path().to_string_lossy().to_string();
    let u: Value = serde_json::from_str(&stdout_of(&["unused", &p, "--json"])).unwrap();
    assert!(u["findings"].is_array());
    let t: Value = serde_json::from_str(&stdout_of(&["top", &p, "--limit", "1", "--json"])).unwrap();
    assert_eq!(t["files"].as_array().unwrap().len(), 1);
    assert_eq!(t["totalMatching"], 2);
}

#[test]
fn drives_json() {
    let v: Value = serde_json::from_str(&stdout_of(&["drives", "--json"])).unwrap();
    let d = v["drives"].as_array().unwrap();
    assert!(!d.is_empty() && d[0]["totalBytes"].as_u64().unwrap() > 0);
    assert!(stdout_of(&["drives"]).contains("ไดรฟ์ในเครื่อง"));
}

#[test]
fn cleanup_needs_yes_and_rejects_unknown_actions() {
    let dry = stdout_of(&["cleanup", "npm-cache"]);
    assert!(dry.contains("--yes") && dry.contains("npm cache clean --force"), "{dry}");
    let dry_json: Value = serde_json::from_str(&stdout_of(&["cleanup", "empty-recycle-bin", "--json"])).unwrap();
    assert_eq!(dry_json["willRun"], false);
    assert_eq!(dry_json["permanent"], true);
    assert!(stdout_of(&["cleanup"]).contains("empty-recycle-bin"));
    let bad = bin().args(["cleanup", "format-c", "--yes"]).output().unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("ไม่รองรับ"));
}

#[test]
fn bad_paths_fail_cleanly() {
    let out = bin().args(["report", "Z:\\definitely\\not\\here"]).output().unwrap();
    assert!(!out.status.success());
    assert!(!String::from_utf8_lossy(&out.stderr).is_empty());
}
