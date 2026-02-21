//! System caches scanner.
//! Finds browser caches, system logs, dev tool caches, and temp files.

use crate::types::{Effort, Finding, ScanResult};
use crate::utils::fs::{get_file_age, get_size, path_exists};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Instant;

/// A known cache target location.
struct CacheTarget {
    /// Path with ~ prefix for home directory.
    path: &'static str,
    /// Human-readable label.
    label: &'static str,
    /// Why this is safe to remove.
    reason: &'static str,
}

/// Known cache locations on macOS.
const CACHE_TARGETS: &[CacheTarget] = &[
    // Browser caches
    CacheTarget { path: "~/Library/Caches/Google/Chrome", label: "Chrome cache", reason: "Browser cache \u{2014} Chrome will rebuild it" },
    CacheTarget { path: "~/Library/Caches/com.google.Chrome", label: "Chrome app cache", reason: "Browser cache \u{2014} Chrome will rebuild it" },
    CacheTarget { path: "~/Library/Caches/Firefox", label: "Firefox cache", reason: "Browser cache \u{2014} Firefox will rebuild it" },
    CacheTarget { path: "~/Library/Caches/com.apple.Safari", label: "Safari cache", reason: "Browser cache \u{2014} Safari will rebuild it" },
    CacheTarget { path: "~/Library/Caches/com.brave.Browser", label: "Brave cache", reason: "Browser cache \u{2014} Brave will rebuild it" },
    CacheTarget { path: "~/Library/Caches/com.microsoft.edgemac", label: "Edge cache", reason: "Browser cache \u{2014} Edge will rebuild it" },
    CacheTarget { path: "~/Library/Caches/com.operasoftware.Opera", label: "Opera cache", reason: "Browser cache \u{2014} Opera will rebuild it" },
    CacheTarget { path: "~/Library/Caches/arc.browser", label: "Arc cache", reason: "Browser cache \u{2014} Arc will rebuild it" },
    // System logs
    CacheTarget { path: "~/Library/Logs", label: "User logs", reason: "Application logs \u{2014} usually safe to clear" },
    // Dev tool caches
    CacheTarget { path: "~/Library/Caches/com.apple.dt.Xcode", label: "Xcode cache", reason: "Xcode derived data cache \u{2014} will be rebuilt" },
    CacheTarget { path: "~/Library/Developer/Xcode/DerivedData", label: "Xcode DerivedData", reason: "Xcode build artifacts \u{2014} will be rebuilt on next build" },
    CacheTarget { path: "~/Library/Developer/Xcode/Archives", label: "Xcode Archives", reason: "Old Xcode build archives \u{2014} usually safe to remove" },
    CacheTarget { path: "~/Library/Developer/CoreSimulator/Caches", label: "iOS Simulator caches", reason: "Simulator caches \u{2014} will be rebuilt" },
    CacheTarget { path: "~/Library/Caches/CocoaPods", label: "CocoaPods cache", reason: "Pod cache \u{2014} will be re-downloaded when needed" },
    CacheTarget { path: "~/Library/Caches/org.carthage.CarthageKit", label: "Carthage cache", reason: "Carthage dependency cache \u{2014} will be re-downloaded" },
    CacheTarget { path: "~/Library/Caches/Homebrew", label: "Homebrew cache", reason: "Downloaded package archives \u{2014} safe to remove" },
    CacheTarget { path: "~/Library/Caches/pip", label: "pip cache", reason: "Python package cache \u{2014} will be re-downloaded" },
    CacheTarget { path: "~/Library/Caches/yarn", label: "Yarn cache", reason: "Yarn package cache \u{2014} will be re-downloaded" },
    CacheTarget { path: "~/.npm/_cacache", label: "npm cache", reason: "npm package cache \u{2014} will be re-downloaded" },
    CacheTarget { path: "~/.bun/install/cache", label: "Bun install cache", reason: "Bun package cache \u{2014} will be re-downloaded" },
    // Temp files
    CacheTarget { path: "~/Library/Caches/com.apple.bird", label: "iCloud temp cache", reason: "iCloud sync temp data \u{2014} will be regenerated" },
    CacheTarget { path: "~/Library/Caches/CloudKit", label: "CloudKit cache", reason: "CloudKit sync cache \u{2014} will be regenerated" },
];

/// Paths that require macOS Full Disk Access (TCC-protected).
static TCC_PROTECTED_PATHS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "~/Library/Caches/com.apple.Safari",
        "~/Library/Caches/com.apple.bird",
        "~/Library/Caches/CloudKit",
    ]
    .into_iter()
    .collect()
});

/// Minimum size to report.
const MIN_SIZE: u64 = 5 * 1024 * 1024; // 5 MB

/// Expand ~ to the user's home directory.
fn expand_home(path: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(path.replacen('~', &home, 1))
}

/// Create and run the system caches scanner.
pub async fn scan() -> ScanResult {
    let start = Instant::now();
    let mut findings = Vec::new();

    let mut handles = Vec::new();

    for target in CACHE_TARGETS {
        let full_path = expand_home(target.path);
        let label = target.label.to_string();
        let reason = target.reason.to_string();
        let is_tcc = TCC_PROTECTED_PATHS.contains(target.path);

        handles.push(tokio::spawn(async move {
            if !path_exists(&full_path).await {
                return None;
            }

            let size = get_size(&full_path).await;
            if size < MIN_SIZE {
                return None;
            }

            let age = get_file_age(&full_path).await;

            let final_reason = if is_tcc {
                format!("{} (may need Full Disk Access)", reason)
            } else {
                reason
            };

            Some(Finding {
                path: full_path.to_string_lossy().to_string(),
                label,
                size,
                age,
                reason: final_reason,
                effort: Some(Effort::None),
            })
        }));
    }

    for handle in handles {
        if let Ok(Some(finding)) = handle.await {
            findings.push(finding);
        }
    }

    // Sort by size descending
    findings.sort_by(|a, b| b.size.cmp(&a.size));

    let total_size = findings.iter().map(|f| f.size).sum();

    ScanResult {
        scanner_name: "System Caches".to_string(),
        findings,
        total_size,
        duration: start.elapsed().as_millis() as u64,
    }
}
