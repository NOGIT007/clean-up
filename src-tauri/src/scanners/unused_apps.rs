//! Unused apps scanner.
//! Detects .app bundles not opened in 6+ months using Spotlight metadata.

use crate::types::Finding;
use crate::types::ScanResult;
use crate::utils::apps::is_system_bundle_id;
use crate::utils::fs::{get_size, safe_readdir};
use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;
use std::time::Instant;
use tokio::process::Command;

/// Months of inactivity before flagging an app.
const STALE_MONTHS: u64 = 6;

/// Minimum app size to bother reporting.
const MIN_SIZE: u64 = 1024 * 1024; // 1 MB

/// Batch size for mdls subprocess calls.
const BATCH_SIZE: usize = 10;

/// Apple apps that should never be flagged as unused.
static SKIP_APPS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "Safari.app",
        "Mail.app",
        "Terminal.app",
        "Activity Monitor.app",
        "System Preferences.app",
        "System Settings.app",
        "App Store.app",
        "Calculator.app",
        "Calendar.app",
        "Contacts.app",
        "Dictionary.app",
        "Disk Utility.app",
        "FaceTime.app",
        "Finder.app",
        "Font Book.app",
        "Home.app",
        "Keychain Access.app",
        "Maps.app",
        "Messages.app",
        "Migration Assistant.app",
        "Music.app",
        "News.app",
        "Notes.app",
        "Photo Booth.app",
        "Photos.app",
        "Podcasts.app",
        "Preview.app",
        "QuickTime Player.app",
        "Reminders.app",
        "Screenshot.app",
        "Shortcuts.app",
        "Siri.app",
        "Stocks.app",
        "TextEdit.app",
        "Time Machine.app",
        "TV.app",
        "Voice Memos.app",
        "Weather.app",
        "Console.app",
        "Automator.app",
        "Books.app",
        "Chess.app",
        "Clock.app",
        "Freeform.app",
        "Grapher.app",
        "Image Capture.app",
        "Launchpad.app",
        "Mission Control.app",
        "Stickies.app",
        "clean-up.app",
        "Clean Up.app",
    ]
    .into_iter()
    .collect()
});

/// Parse a datetime string like "2024-01-15T10:30:00Z" to ms since epoch.
fn parse_datetime_to_ms(s: &str) -> Option<u64> {
    // Simple manual parse: YYYY-MM-DDTHH:MM:SSZ
    // We avoid pulling in chrono by doing basic parsing
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let year: i64 = s[0..4].parse().ok()?;
    let month: i64 = s[5..7].parse().ok()?;
    let day: i64 = s[8..10].parse().ok()?;
    let hour: i64 = s[11..13].parse().ok()?;
    let min: i64 = s[14..16].parse().ok()?;
    let sec: i64 = s[17..19].parse().ok()?;

    // Simplified days-since-epoch calculation (good enough for "months ago" comparisons)
    // Using a rough approximation - days from year 1970
    let mut days: i64 = 0;
    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }
    let month_days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..month {
        days += month_days[(m - 1) as usize] as i64;
        if m == 2 && is_leap_year(year) {
            days += 1;
        }
    }
    days += day - 1;

    let total_secs = days * 86400 + hour * 3600 + min * 60 + sec;
    Some((total_secs * 1000) as u64)
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Get the last-used date of an app via Spotlight metadata (ms since epoch).
async fn get_last_used_date(app_path: &str) -> Option<u64> {
    let output = Command::new("mdls")
        .args(["-name", "kMDItemLastUsedDate", app_path])
        .output()
        .await
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);

    // Parse: kMDItemLastUsedDate = 2024-01-15 10:30:00 +0000
    let re = regex::Regex::new(
        r"kMDItemLastUsedDate\s*=\s*(\d{4}-\d{2}-\d{2}) (\d{2}:\d{2}:\d{2})",
    )
    .ok()?;
    let caps = re.captures(&text)?;
    let date = caps.get(1)?.as_str();
    let time = caps.get(2)?.as_str();

    let datetime_str = format!("{}T{}Z", date, time);
    parse_datetime_to_ms(&datetime_str)
}

/// Get the bundle ID of an app via Spotlight metadata.
async fn get_bundle_id(app_path: &str) -> Option<String> {
    let output = Command::new("mdls")
        .args(["-name", "kMDItemCFBundleIdentifier", app_path])
        .output()
        .await
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let start = text.find('"')? + 1;
    let end = text[start..].find('"')? + start;
    Some(text[start..end].to_string())
}

/// Format a duration in months (rounded).
fn format_months(ms: u64) -> String {
    let months = (ms as f64 / (1000.0 * 60.0 * 60.0 * 24.0 * 30.0)).round() as u64;
    match months {
        0 => "less than a month".to_string(),
        1 => "1 month".to_string(),
        n => format!("{} months", n),
    }
}

/// Extract the .app name from a full path.
fn app_name(app_path: &str) -> String {
    app_path
        .rsplit('/')
        .next()
        .unwrap_or(app_path)
        .to_string()
}

/// Get current time as ms since epoch.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Create and run the unused apps scanner.
pub async fn scan() -> ScanResult {
    let start = Instant::now();
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            return ScanResult {
                scanner_name: "Unused Apps".to_string(),
                findings: Vec::new(),
                total_size: 0,
                duration: 0,
            };
        }
    };

    // Gather .app paths from both locations
    let app_dirs = vec!["/Applications".to_string(), format!("{}/Applications", home)];
    let mut app_paths = Vec::new();

    for dir in &app_dirs {
        let entries = safe_readdir(Path::new(dir)).await;
        for entry in entries {
            let path_str = entry.to_string_lossy().to_string();
            if path_str.ends_with(".app") {
                app_paths.push(path_str);
            }
        }
    }

    let mut findings = Vec::new();
    let cutoff_ms = STALE_MONTHS * 30 * 24 * 60 * 60 * 1000;
    let current_ms = now_ms();
    let cutoff_time = current_ms.saturating_sub(cutoff_ms);

    // Process in batches
    for batch in app_paths.chunks(BATCH_SIZE) {
        let mut handles = Vec::new();

        for app_path in batch {
            let app_path = app_path.clone();
            let cutoff_time = cutoff_time;
            let current_ms = current_ms;

            handles.push(tokio::spawn(async move {
                let name = app_name(&app_path);

                // Skip known Apple apps
                if SKIP_APPS.contains(name.as_str()) {
                    return None;
                }

                // Check bundle ID for system apps
                if let Some(bundle_id) = get_bundle_id(&app_path).await {
                    if is_system_bundle_id(&bundle_id) {
                        return None;
                    }
                }

                // Skip tiny apps
                let size = get_size(Path::new(&app_path)).await;
                if size < MIN_SIZE {
                    return None;
                }

                // Check last used date
                if let Some(last_used_ms) = get_last_used_date(&app_path).await {
                    if last_used_ms >= cutoff_time {
                        return None; // Used recently
                    }

                    let age = current_ms.saturating_sub(last_used_ms);
                    Some(Finding {
                        path: app_path,
                        label: name.trim_end_matches(".app").to_string(),
                        size,
                        age,
                        reason: format!("Not opened in {}", format_months(age)),
                        effort: None,
                    })
                } else {
                    // No usage data
                    let age = current_ms.saturating_sub(cutoff_time);
                    Some(Finding {
                        path: app_path,
                        label: name.trim_end_matches(".app").to_string(),
                        size,
                        age,
                        reason: "No usage data \u{2014} may be unused".to_string(),
                        effort: None,
                    })
                }
            }));
        }

        for handle in handles {
            if let Ok(Some(finding)) = handle.await {
                findings.push(finding);
            }
        }
    }

    // Sort by size descending
    findings.sort_by(|a, b| b.size.cmp(&a.size));

    let total_size = findings.iter().map(|f| f.size).sum();

    ScanResult {
        scanner_name: "Unused Apps".to_string(),
        findings,
        total_size,
        duration: start.elapsed().as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_datetime_basic() {
        // 2024-01-01T00:00:00Z should be some positive value
        let ms = parse_datetime_to_ms("2024-01-01T00:00:00Z");
        assert!(ms.is_some());
        assert!(ms.unwrap() > 0);
    }

    #[test]
    fn parse_datetime_known_epoch() {
        // 1970-01-01T00:00:00Z should be 0
        let ms = parse_datetime_to_ms("1970-01-01T00:00:00Z");
        assert_eq!(ms, Some(0));
    }

    #[test]
    fn parse_datetime_with_time() {
        // 1970-01-01T01:00:00Z = 3600 seconds = 3600000 ms
        let ms = parse_datetime_to_ms("1970-01-01T01:00:00Z");
        assert_eq!(ms, Some(3_600_000));
    }

    #[test]
    fn parse_datetime_invalid() {
        assert_eq!(parse_datetime_to_ms("not a date"), None);
        assert_eq!(parse_datetime_to_ms(""), None);
        assert_eq!(parse_datetime_to_ms("2024"), None);
    }

    #[test]
    fn leap_year_detection() {
        assert!(is_leap_year(2000)); // divisible by 400
        assert!(is_leap_year(2024)); // divisible by 4, not 100
        assert!(!is_leap_year(1900)); // divisible by 100, not 400
        assert!(!is_leap_year(2023)); // not divisible by 4
    }

    #[test]
    fn format_months_display() {
        assert_eq!(format_months(0), "less than a month");
        let one_month_ms = 30 * 24 * 60 * 60 * 1000;
        assert_eq!(format_months(one_month_ms), "1 month");
        assert_eq!(format_months(one_month_ms * 6), "6 months");
    }

    #[test]
    fn app_name_extraction() {
        assert_eq!(app_name("/Applications/Firefox.app"), "Firefox.app");
        assert_eq!(app_name("/Users/test/Applications/MyApp.app"), "MyApp.app");
        assert_eq!(app_name("StandaloneApp.app"), "StandaloneApp.app");
    }

    #[test]
    fn skip_apps_includes_system_apps() {
        assert!(SKIP_APPS.contains("Safari.app"));
        assert!(SKIP_APPS.contains("Finder.app"));
        assert!(SKIP_APPS.contains("Clean Up.app"));
        assert!(!SKIP_APPS.contains("Firefox.app"));
    }
}
