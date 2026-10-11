//! Command line interface: Thai human output by default, `--json` for machines.

use crate::engine::{self, cleanup_reply};
use crate::views::{self, Reply};
use crate::{fmt, mcp};
use clap::{Parser, Subcommand};
use diskwhy_core::actions;
use diskwhy_core::insights;
use diskwhy_core::report::{self, ScanResult};
use diskwhy_core::scan::Progress;
use serde_json::Value;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(
    name = "diskwhy-cli",
    version,
    about = "DiskWhy: ทำไมดิสก์เต็ม? วิเคราะห์ จัดกลุ่มตามความปลอดภัย และเก็บกวาด Windows อย่างปลอดภัย / Explain why a Windows disk is full",
    long_about = "DiskWhy: ทำไมดิสก์เต็ม? สแกนไดรฟ์ หาตัวกินพื้นที่และข้อมูลที่ไม่ได้ใช้ แล้วเก็บกวาดอย่างปลอดภัย (ย้ายไปถังขยะเท่านั้น ไม่ลบถาวร ยกเว้น cleanup empty-recycle-bin)\n\
        Explain why a Windows disk is full: scan a drive, find space hogs and unused data, and clean up safely (Recycle Bin only; the one permanent action is `cleanup empty-recycle-bin`).\n\n\
        ใช้กับ AI ได้: `diskwhy-cli mcp` เปิด MCP server (stdio) ให้ Claude Code / Claude Desktop เรียกใช้เครื่องมือเหล่านี้แทนคุณ\n\
        Use with an AI assistant: `diskwhy-cli mcp` runs an MCP server (stdio) for Claude Code / Claude Desktop.",
    after_help = "ตัวอย่าง / Examples:\n  diskwhy-cli drives\n  diskwhy-cli report C:\\ --top 10\n  diskwhy-cli unused C:\\Users --json\n  diskwhy-cli top D:\\ --limit 30\n  diskwhy-cli cleanup npm-cache --yes\n  diskwhy-cli mcp"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// แสดงไดรฟ์ทั้งหมด พื้นที่ว่าง และไดรฟ์ที่เกือบเต็ม / List drives with free space
    Drives {
        /// พิมพ์ผลเป็น JSON / Print JSON
        #[arg(long)]
        json: bool,
    },
    /// สแกนแล้วสรุปรายงานเต็ม: หมวดไฟล์ ตัวกินพื้นที่ และข้อมูลที่ไม่ได้ใช้ / Scan and print the full report
    Report {
        /// ไดรฟ์หรือโฟลเดอร์ เช่น C:\ หรือ C:\Users / Drive or folder to scan
        path: PathBuf,
        /// พิมพ์ผลเป็น JSON / Print JSON
        #[arg(long)]
        json: bool,
        /// จำนวนรายการสูงสุดที่แสดง (ตัวกินพื้นที่และไฟล์ใหญ่) / Max findings and largest files to show
        #[arg(long, default_value_t = 10, value_name = "N")]
        top: usize,
    },
    /// หาข้อมูลที่ไม่ได้ใช้: ไฟล์ใหญ่เก่า โปรเจกต์เก่า ตัวติดตั้งเก่า AppData ที่ตกค้าง / Find unused or stale data
    Unused {
        path: PathBuf,
        /// พิมพ์ผลเป็น JSON / Print JSON
        #[arg(long)]
        json: bool,
    },
    /// ไฟล์ใหญ่ที่สุด / Largest files
    Top {
        path: PathBuf,
        /// จำนวนไฟล์ / Number of files
        #[arg(long, default_value_t = 20, value_name = "N")]
        limit: usize,
        /// พิมพ์ผลเป็น JSON / Print JSON
        #[arg(long)]
        json: bool,
    },
    /// รันการเก็บกวาดที่อนุญาตไว้ (ไม่ใส่ --yes = แค่แสดงว่าจะทำอะไร) / Run a whitelisted cleanup action (without --yes it only shows what it would do)
    Cleanup {
        /// action: npm-cache, pnpm-store, yarn-cache, pip-cache, nuget-cache, wsl-shutdown, temp-files, empty-recycle-bin, open-storage-sense, open-disk-cleanup, open-apps-features, open-recycle-bin (ไม่ระบุ = แสดงรายการ)
        action: Option<String>,
        /// ยืนยันให้ทำจริง / Really run it
        #[arg(long)]
        yes: bool,
        /// พิมพ์ผลเป็น JSON / Print JSON
        #[arg(long)]
        json: bool,
    },
    /// เปิด MCP server (stdio) ให้ AI อย่าง Claude Code / Claude Desktop เรียกใช้ / Run the MCP server over stdio
    Mcp,
}

pub fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("diskwhy-cli: {e}");
            ExitCode::FAILURE
        }
    }
}

pub fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Cmd::Drives { json } => emit(views::drives_reply(), json),
        Cmd::Report { path, json, top } => {
            let result = scan_blocking(&path)?;
            let hogs = insights::find_hogs(&result.root, &result.root_path);
            let now = fmt::now_secs();
            let top = top.max(1);
            if json {
                let mut rep = views::report_reply(&result, &hogs, top, 8, now);
                let files = views::largest_reply(&result, top, None, None, now);
                rep.data["topFiles"] = files.data["files"].clone();
                print_json(&rep.data)
            } else {
                let rep = views::report_reply(&result, &hogs, top, 5, now);
                let files = views::largest_reply(&result, top, None, None, now);
                println!("{}\n\n{}", rep.text, files.text);
                Ok(())
            }
        }
        Cmd::Unused { path, json } => {
            let result = scan_blocking(&path)?;
            let hogs = insights::find_hogs(&result.root, &result.root_path);
            emit(views::unused_reply(&result, &hogs, if json { 30 } else { 10 }, fmt::now_secs()), json)
        }
        Cmd::Top { path, limit, json } => {
            let result = scan_blocking(&path)?;
            emit(views::largest_reply(&result, limit.max(1), None, None, fmt::now_secs()), json)
        }
        Cmd::Cleanup { action, yes, json } => cleanup(action.as_deref(), yes, json),
        Cmd::Mcp => mcp::run_stdio(),
    }
}

fn print_json(v: &Value) -> Result<(), String> {
    println!("{}", serde_json::to_string_pretty(v).map_err(|e| e.to_string())?);
    Ok(())
}

/// Print a reply (text or JSON); an error reply becomes a failing exit code.
fn emit(reply: Reply, json: bool) -> Result<(), String> {
    if reply.is_error {
        return Err(reply.text);
    }
    if json { print_json(&reply.data) } else {
        println!("{}", reply.text);
        Ok(())
    }
}

/// Scan `path` on this thread, showing progress on stderr when it is a terminal.
pub fn scan_blocking(path: &std::path::Path) -> Result<ScanResult, String> {
    let given = engine::normalize_path(&path.to_string_lossy())?;
    if !given.is_dir() {
        return Err(format!("ไม่พบโฟลเดอร์ {} หรือไม่ใช่โฟลเดอร์", given.display()));
    }
    let root = PathBuf::from(engine::canonical_string(&given).unwrap_or_else(|| given.to_string_lossy().to_string()));
    let (progress, cancel, done) = (Progress::default(), AtomicBool::new(false), AtomicBool::new(false));
    let show = std::io::stderr().is_terminal();
    let result = std::thread::scope(|s| {
        if show {
            s.spawn(|| {
                while !done.load(Relaxed) {
                    let cur = progress.current.try_lock().map(|c| c.clone()).unwrap_or_default();
                    eprint!("\rกำลังสแกน... {} ไฟล์, {}  {:<60}", progress.files.load(Relaxed), fmt::bytes(progress.bytes.load(Relaxed)), fmt::shorten(&cur, 60));
                    std::thread::sleep(Duration::from_millis(400));
                }
                eprint!("\r{:<110}\r", "");
            });
        }
        let r = report::run_scan(&root, &progress, &cancel);
        done.store(true, Relaxed);
        r
    });
    result.ok_or_else(|| "การสแกนถูกยกเลิก".to_string())
}

fn cleanup(action: Option<&str>, yes: bool, json: bool) -> Result<(), String> {
    let Some(action) = action else {
        let list: Vec<Value> = actions::CLEANUP_ACTIONS
            .iter()
            .map(|a| serde_json::json!({ "action": a.id, "title": a.title, "description": a.description, "destructive": a.destructive, "permanent": a.permanent, "commands": actions::describe_commands(a) }))
            .collect();
        if json {
            return print_json(&Value::Array(list));
        }
        println!("action ที่รันได้ (ใช้: diskwhy-cli cleanup <action> --yes):");
        for a in actions::CLEANUP_ACTIONS {
            println!("  {:<20} {}{}", a.id, a.title, if a.permanent { "  [ลบถาวร]" } else { "" });
        }
        return Ok(());
    };
    let spec = actions::cleanup_spec(action.trim())
        .ok_or_else(|| format!("ไม่รองรับ action {action:?} — ใช้ได้เฉพาะ: {}", actions::cleanup_ids().join(", ")))?;
    if !yes {
        let cmds = actions::describe_commands(spec);
        if json {
            return print_json(&serde_json::json!({
                "action": spec.id, "title": spec.title, "description": spec.description,
                "destructive": spec.destructive, "permanent": spec.permanent, "commands": cmds, "willRun": false,
                "hint": "add --yes to run"
            }));
        }
        println!("จะทำ: {} ({})\n{}\nคำสั่ง:", spec.title, spec.id, spec.description);
        for c in cmds {
            println!("  {c}");
        }
        println!("{}\n(ยังไม่ได้ทำอะไร — เพิ่ม --yes เพื่อรันจริง)", if spec.permanent { "คำเตือน: การกระทำนี้ลบถาวร กู้คืนไม่ได้" } else { "" });
        return Ok(());
    }
    let reply = cleanup_reply(spec.id, actions::run_cleanup(spec.id), false);
    let failed = reply.is_error;
    if json { print_json(&reply.data)?; } else { println!("{}", reply.text); }
    if failed { Err("การเก็บกวาดไม่สำเร็จ".into()) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_every_subcommand() {
        let p = |a: &[&str]| Cli::try_parse_from(std::iter::once("diskwhy-cli").chain(a.iter().copied()));
        assert!(matches!(p(&["drives"]).unwrap().command, Cmd::Drives { json: false }));
        assert!(matches!(p(&["drives", "--json"]).unwrap().command, Cmd::Drives { json: true }));
        match p(&["report", "C:\\", "--json", "--top", "5"]).unwrap().command {
            Cmd::Report { top, json, path } => assert_eq!((top, json, path), (5, true, PathBuf::from("C:\\"))),
            c => panic!("{c:?}"),
        }
        assert!(matches!(p(&["report", "C:\\"]).unwrap().command, Cmd::Report { top: 10, .. }));
        assert!(matches!(p(&["unused", "C:\\"]).unwrap().command, Cmd::Unused { .. }));
        assert!(matches!(p(&["top", "C:\\", "--limit", "7"]).unwrap().command, Cmd::Top { limit: 7, .. }));
        match p(&["cleanup", "npm-cache", "--yes"]).unwrap().command {
            Cmd::Cleanup { action, yes, .. } => assert_eq!((action.as_deref(), yes), (Some("npm-cache"), true)),
            c => panic!("{c:?}"),
        }
        assert!(matches!(p(&["cleanup", "npm-cache"]).unwrap().command, Cmd::Cleanup { yes: false, .. }));
        assert!(matches!(p(&["mcp"]).unwrap().command, Cmd::Mcp));
        assert!(p(&["report"]).is_err(), "path is required");
        assert!(p(&["bogus"]).is_err());
    }

    #[test]
    fn cleanup_without_yes_and_unknown_actions_never_run_anything() {
        assert!(cleanup(Some("npm-cache"), false, true).is_ok());
        assert!(cleanup(Some("empty-recycle-bin"), false, false).is_ok());
        assert!(cleanup(None, false, false).is_ok());
        let err = cleanup(Some("format-c"), true, false).unwrap_err();
        assert!(err.contains("ไม่รองรับ"));
    }

    #[test]
    fn scan_blocking_scans_a_temp_dir_and_rejects_bad_paths() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.bin"), vec![0u8; 2 * 1024 * 1024]).unwrap();
        let r = scan_blocking(d.path()).unwrap();
        assert_eq!(r.stats.bytes, 2 * 1024 * 1024);
        assert!(scan_blocking(&d.path().join("missing")).is_err());
        assert!(scan_blocking(std::path::Path::new("relative")).is_err());
    }
}
