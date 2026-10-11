//! End-to-end: spawn the real `diskwhy-cli mcp` binary and speak newline-delimited JSON-RPC over stdio.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

struct Client {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
    next_id: u64,
    /// Every raw stdout line, to prove stdout carries protocol only.
    seen: Vec<String>,
}

impl Client {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_diskwhy-cli"))
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn diskwhy-cli mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Client { child, stdin, rx, next_id: 1, seen: vec![] }
    }

    fn send(&mut self, v: Value) {
        writeln!(self.stdin, "{v}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn notify(&mut self, method: &str) {
        self.send(json!({ "jsonrpc": "2.0", "method": method }));
    }

    /// Send a request and wait for the response with the same id.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(line) => {
                    self.seen.push(line.clone());
                    let v: Value = serde_json::from_str(&line).unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line}"));
                    if v["id"] == json!(id) {
                        return v;
                    }
                }
                Err(RecvTimeoutError::Timeout) => panic!("timeout waiting for {method}"),
                Err(RecvTimeoutError::Disconnected) => panic!("server closed stdout during {method}"),
            }
        }
    }

    fn call(&mut self, tool: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": tool, "arguments": args }))
    }

    fn handshake(&mut self) -> Value {
        let init = self.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "diskwhy-test", "version": "0.0.1" } }),
        );
        self.notify("notifications/initialized");
        init
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn structured(resp: &Value) -> &Value {
    &resp["result"]["structuredContent"]
}

fn text(resp: &Value) -> String {
    resp["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string()
}

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    std::fs::create_dir_all(r.join("proj/node_modules/lib")).unwrap();
    std::fs::write(r.join("proj/package.json"), b"{}").unwrap();
    std::fs::write(r.join("proj/node_modules/lib/big.bin"), vec![1u8; 3 * 1024 * 1024]).unwrap();
    std::fs::write(r.join("movie.mp4"), vec![2u8; 2 * 1024 * 1024]).unwrap();
    dir
}

/// Canonical path text without the `\\?\` prefix (the server canonicalises the scan root the same way).
fn canon(p: &std::path::Path) -> String {
    let c = std::fs::canonicalize(p).unwrap().to_string_lossy().to_string();
    c.strip_prefix("\\\\?\\").unwrap_or(&c).to_string()
}

#[test]
fn full_mcp_session_over_stdio() {
    let dir = fixture();
    let root = canon(dir.path());
    let mut c = Client::spawn();

    // initialize
    let init = c.handshake();
    let result = &init["result"];
    assert_eq!(result["serverInfo"]["name"], "diskwhy");
    assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(result["capabilities"]["tools"].is_object() && result["capabilities"]["prompts"].is_object());
    let instructions = result["instructions"].as_str().unwrap();
    assert!(instructions.contains("move_to_recycle_bin") && instructions.contains("Thai"));

    // tools/list: names + annotations
    let list = c.request("tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["admin_commands", "browse", "cancel_scan", "disk_report", "find_unused", "largest_files", "list_drives", "move_to_recycle_bin", "run_cleanup", "scan", "scan_status"]
    );
    for t in tools {
        let a = &t["annotations"];
        let name = t["name"].as_str().unwrap();
        assert!(a.is_object(), "{name} lacks annotations");
        assert_eq!(a["openWorldHint"], false);
        let destructive = matches!(name, "move_to_recycle_bin" | "run_cleanup");
        assert_eq!(a["destructiveHint"] == true, destructive, "{name}");
        assert_eq!(a["readOnlyHint"] == true, !destructive && name != "cancel_scan", "{name}");
    }

    // list_drives
    let drives = c.call("list_drives", json!({}));
    assert_eq!(drives["result"]["isError"], false, "{drives}");
    assert!(!structured(&drives)["drives"].as_array().unwrap().is_empty());

    // tools need a scan first -> a helpful tool error, not a protocol error
    let early = c.call("disk_report", json!({}));
    assert_eq!(early["result"]["isError"], true);
    assert!(text(&early).contains("scan"));

    // scan lifecycle
    let scan = c.call("scan", json!({ "path": root, "wait_seconds": 30 }));
    assert_eq!(scan["result"]["isError"], false, "{scan}");
    let s = structured(&scan);
    assert_eq!(s["status"], "done", "{scan}");
    assert_eq!(s["files"], 3);
    assert_eq!(s["totalBytes"], 5 * 1024 * 1024 + 2);
    let status = c.call("scan_status", json!({}));
    assert_eq!(structured(&status)["status"], "done");

    // disk_report -> structured JSON parses and has the contract fields
    let rep = c.call("disk_report", json!({ "max_findings": 10, "max_locations": 3 }));
    assert_eq!(rep["result"]["isError"], false, "{rep}");
    let r = structured(&rep);
    assert!(r["findings"].is_array() && r["categories"].is_array());
    assert!(r["safeToReclaimBytes"].as_u64().unwrap() >= 3 * 1024 * 1024);
    for f in r["findings"].as_array().unwrap() {
        assert!(["safe", "review", "system"].contains(&f["safety"].as_str().unwrap()));
        assert!(f["id"].is_string() && f["size"].is_string() && f["advice"].is_string());
    }
    assert!(text(&rep).contains("รายงานพื้นที่ดิสก์"));

    // other read-only tools
    assert_eq!(structured(&c.call("browse", json!({})))["children"][0]["name"], "proj");
    let big = c.call("largest_files", json!({ "limit": 1 }));
    assert_eq!(structured(&big)["files"][0]["name"], "big.bin");
    assert_eq!(c.call("find_unused", json!({}))["result"]["isError"], false);
    let admin = c.call("admin_commands", json!({}));
    assert!(text(&admin).contains("DISM /Online /Cleanup-Image /StartComponentCleanup"));

    // safety: protected path and an unknown cleanup are tool errors; nothing was touched
    let prot = c.call("move_to_recycle_bin", json!({ "paths": ["C:\\Windows\\System32", "C:\\"] }));
    assert_eq!(prot["result"]["isError"], true);
    assert!(structured(&prot)["results"].as_array().unwrap().iter().all(|r| r["status"] == "refused"));
    let outside_path = std::env::temp_dir().join("diskwhy-not-in-scan.txt");
    std::fs::write(&outside_path, b"x").unwrap();
    let outside = c.call("move_to_recycle_bin", json!({ "paths": [outside_path.to_string_lossy()] }));
    assert_eq!(outside["result"]["isError"], true);
    assert!(outside_path.exists(), "a file outside the scan root must not be touched");
    let _ = std::fs::remove_file(&outside_path);
    let bad = c.call("run_cleanup", json!({ "action": "format-c" }));
    assert_eq!(bad["result"]["isError"], true);
    assert!(text(&bad).contains("ไม่รองรับ"));
    assert!(dir.path().join("movie.mp4").exists() && dir.path().join("proj/node_modules/lib/big.bin").exists());

    // prompts
    let prompts = c.request("prompts/list", json!({}));
    let pn: Vec<&str> = prompts["result"]["prompts"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert!(pn.contains(&"diagnose-disk") && pn.contains(&"cleanup-quick-wins"), "{pn:?}");
    let got = c.request("prompts/get", json!({ "name": "diagnose-disk", "arguments": { "drive": "D:\\" } }));
    let body = got["result"]["messages"][0]["content"]["text"].as_str().unwrap();
    assert!(body.contains("list_drives") && body.contains("D:\\") && body.contains("untrusted"));
    let quick = c.request("prompts/get", json!({ "name": "cleanup-quick-wins" }));
    assert!(quick["result"]["messages"][0]["content"]["text"].as_str().unwrap().contains("safe"));

    // ping + unknown method must not crash the server
    assert!(c.request("ping", json!({})).get("error").is_none());
    assert!(c.request("does/not/exist", json!({})).get("error").is_some());
    assert!(c.call("list_drives", json!({}))["result"].is_object(), "server is still alive");

    // stdout carried JSON-RPC only
    assert!(c.seen.iter().all(|l| serde_json::from_str::<Value>(l).is_ok()));
}

#[test]
fn scan_can_be_polled_and_cancelled() {
    // On a fast disk a tiny scan may already be done when wait_seconds=0 returns, so accept either state.
    let dir = fixture();
    let root = canon(dir.path());
    let mut c = Client::spawn();
    c.handshake();
    let first = c.call("scan", json!({ "path": root, "wait_seconds": 0 }));
    let st = structured(&first)["status"].as_str().unwrap().to_string();
    assert!(st == "running" || st == "done", "{st}");
    let mut done = st == "done";
    for _ in 0..100 {
        if done {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        done = structured(&c.call("scan_status", json!({})))["status"] == "done";
    }
    assert!(done, "scan finished");
    let cancel = c.call("cancel_scan", json!({}));
    assert_eq!(cancel["result"]["isError"], false);
}

#[test]
fn bad_scan_paths_are_tool_errors() {
    let mut c = Client::spawn();
    c.handshake();
    for path in ["", "relative\\path", "Z:\\definitely\\not\\here"] {
        let r = c.call("scan", json!({ "path": path, "wait_seconds": 1 }));
        assert_eq!(r["result"]["isError"], true, "{path:?}: {r}");
    }
    let missing_arg = c.call("scan", json!({}));
    assert!(missing_arg.get("error").is_some() || missing_arg["result"]["isError"] == true);
}
