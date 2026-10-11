//! Human-readable formatting shared by the CLI and the MCP tool text (Thai).

use std::time::{SystemTime, UNIX_EPOCH};

/// Binary units like Windows Explorer: "512 B", "12.4 GB".
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    if n < 1024 {
        return format!("{n} B");
    }
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if v >= 100.0 { format!("{v:.0} {}", UNITS[u]) } else { format!("{v:.1} {}", UNITS[u]) }
}

pub fn percent(part: u64, whole: u64) -> f64 {
    if whole == 0 { 0.0 } else { ((part as f64 / whole as f64) * 1000.0).round() / 10.0 }
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Whole days between a unix timestamp and `now`; None if the timestamp is unknown (0).
pub fn age_days(now: u64, t: u64) -> Option<u64> {
    (t > 0).then(|| now.saturating_sub(t) / 86_400)
}

/// "วันนี้", "12 วันก่อน", "5 เดือนก่อน", "2 ปีก่อน".
pub fn age_text(days: Option<u64>) -> String {
    match days {
        None => "ไม่ทราบ".into(),
        Some(0) => "วันนี้".into(),
        Some(d) if d < 60 => format!("{d} วันก่อน"),
        Some(d) if d < 730 => format!("{} เดือนก่อน", d / 30),
        Some(d) => format!("{} ปีก่อน", d / 365),
    }
}

pub fn seconds(ms: u64) -> String {
    let s = ms as f64 / 1000.0;
    if s < 90.0 { format!("{s:.1} วินาที") } else { format!("{:.1} นาที", s / 60.0) }
}

/// Shorten a long path in the middle for one-line display.
pub fn shorten(path: &str, max: usize) -> String {
    let n = path.chars().count();
    if n <= max {
        return path.to_string();
    }
    let keep = max.saturating_sub(1);
    let head: String = path.chars().take(keep / 3).collect();
    let tail: String = path.chars().skip(n - (keep - keep / 3)).collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes() {
        assert_eq!(bytes(0), "0 B");
        assert_eq!(bytes(1023), "1023 B");
        assert_eq!(bytes(1024), "1.0 KB");
        assert_eq!(bytes(13_314_398_618), "12.4 GB");
        assert_eq!(bytes(300 * 1024 * 1024), "300 MB");
        assert_eq!(bytes(5 * 1024u64.pow(4)), "5.0 TB");
    }

    #[test]
    fn formats_ages() {
        assert_eq!(age_text(Some(0)), "วันนี้");
        assert_eq!(age_text(Some(12)), "12 วันก่อน");
        assert_eq!(age_text(Some(150)), "5 เดือนก่อน");
        assert_eq!(age_text(Some(1000)), "2 ปีก่อน");
        assert_eq!(age_text(None), "ไม่ทราบ");
        assert_eq!(age_days(1_000_000, 0), None);
        assert_eq!(age_days(86_400 * 3, 86_400), Some(2));
    }

    #[test]
    fn percent_and_shorten() {
        assert_eq!(percent(1, 3), 33.3);
        assert_eq!(percent(1, 0), 0.0);
        let s = shorten(r"C:\Users\verylongname\AppData\Local\Something\Else\file.bin", 30);
        assert!(s.chars().count() <= 30 && s.contains('…'));
        assert_eq!(shorten("short", 30), "short");
    }
}
