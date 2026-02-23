//! App leftovers scanner.
//! Finds orphaned application data in ~/Library for apps that are
//! no longer installed. Skips Apple system services.

use crate::types::{Effort, Finding, ScanResult};
use crate::utils::apps::{extract_bundle_id, get_installed_apps, is_system_bundle_id};
use crate::utils::fs::{get_file_age, get_size, safe_readdir};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Instant;

/// ~/Library subdirectories to scan for orphaned app data.
const LIBRARY_SUBDIRS: &[&str] = &[
    "Application Support",
    "Caches",
    "Containers",
    "Logs",
    "Preferences",
    "Saved Application State",
    "WebKit",
    "HTTPStorages",
];

/// Minimum size to report (skip tiny preference files).
const MIN_SIZE: u64 = 512 * 1024; // 512 KB

/// Additional names to always skip (not bundle-ID based but safe).
static ALWAYS_SKIP: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [".ds_store", ".localized", "apple", "com.apple.appstore"]
        .into_iter()
        .collect()
});

/// Create and run the app leftovers scanner.
pub async fn scan() -> ScanResult {
    let start = Instant::now();
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            return ScanResult {
                scanner_name: "App Leftovers".to_string(),
                findings: Vec::new(),
                total_size: 0,
                duration: 0,
            };
        }
    };

    // Get currently installed apps
    let installed_apps = Arc::new(get_installed_apps().await);
    let mut handles = Vec::new();

    for subdir in LIBRARY_SUBDIRS {
        let dir_path = PathBuf::from(&home).join("Library").join(subdir);
        let installed_apps = Arc::clone(&installed_apps);
        let subdir = *subdir;

        handles.push(tokio::spawn(async move {
            let entries = safe_readdir(&dir_path).await;
            let mut findings = Vec::new();

            for entry_path in &entries {
                let name = match entry_path.file_name() {
                    Some(n) => n.to_string_lossy().to_string(),
                    None => continue,
                };

                if name.starts_with('.') {
                    continue;
                }
                if ALWAYS_SKIP.contains(name.to_lowercase().as_str()) {
                    continue;
                }

                let bundle_id = match extract_bundle_id(&name) {
                    Some(id) => id,
                    None => continue,
                };

                if is_system_bundle_id(&bundle_id) {
                    continue;
                }

                if installed_apps.contains(&bundle_id) {
                    continue;
                }

                let size = get_size(entry_path).await;
                if size < MIN_SIZE {
                    continue;
                }

                let age = get_file_age(entry_path).await;

                findings.push(Finding {
                    path: entry_path.to_string_lossy().to_string(),
                    label: format!("{} in ~/Library/{}", name, subdir),
                    size,
                    age,
                    reason: format!("Data from uninstalled app ({})", bundle_id),
                    effort: Some(Effort::None),
                });
            }

            findings
        }));
    }

    let mut findings = Vec::new();
    for handle in handles {
        if let Ok(subdir_findings) = handle.await {
            findings.extend(subdir_findings);
        }
    }

    // Sort by size descending
    findings.sort_by(|a, b| b.size.cmp(&a.size));

    let total_size = findings.iter().map(|f| f.size).sum();

    ScanResult {
        scanner_name: "App Leftovers".to_string(),
        findings,
        total_size,
        duration: start.elapsed().as_millis() as u64,
    }
}
