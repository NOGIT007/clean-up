//! Scanner modules: each scanner finds cleanable items in a specific category.

pub mod app_leftovers;
pub mod dev_artifacts;
pub mod homebrew_cleanup;
pub mod large_old_files;
pub mod system_caches;
pub mod unused_apps;

use crate::types::{ScanResult, ScannerInfo};

/// All available scanner definitions.
const SCANNER_DEFS: &[(&str, &str, &str)] = &[
    (
        "dev-artifacts",
        "Dev Artifacts",
        "node_modules, build caches, virtual environments",
    ),
    (
        "system-caches",
        "System Caches",
        "Browser caches, system logs, dev tool caches",
    ),
    (
        "app-leftovers",
        "App Leftovers",
        "Orphaned data from uninstalled applications",
    ),
    (
        "large-old-files",
        "Large & Old Files",
        "Files >100MB or untouched for >1 year",
    ),
    (
        "unused-apps",
        "Unused Apps",
        "Applications not opened in 6+ months",
    ),
    (
        "homebrew-cleanup",
        "Homebrew Cleanup",
        "Old formula versions and stale Homebrew cache",
    ),
];

/// Get metadata for all available scanners.
pub fn all_scanner_info() -> Vec<ScannerInfo> {
    SCANNER_DEFS
        .iter()
        .map(|(id, name, desc)| ScannerInfo {
            id: id.to_string(),
            name: name.to_string(),
            description: desc.to_string(),
        })
        .collect()
}

/// Run selected scanners by their IDs.
/// If scanner_ids is empty, runs all scanners.
pub async fn run_scanners(scanner_ids: &[String]) -> Vec<ScanResult> {
    let all_ids: Vec<String> = if scanner_ids.is_empty() {
        SCANNER_DEFS.iter().map(|(id, _, _)| id.to_string()).collect()
    } else {
        scanner_ids.to_vec()
    };

    let mut handles = Vec::new();

    for id in &all_ids {
        let id = id.clone();
        handles.push(tokio::spawn(async move {
            match id.as_str() {
                "dev-artifacts" => dev_artifacts::scan().await,
                "system-caches" => system_caches::scan().await,
                "app-leftovers" => app_leftovers::scan().await,
                "large-old-files" => large_old_files::scan().await,
                "unused-apps" => unused_apps::scan().await,
                "homebrew-cleanup" => homebrew_cleanup::scan().await,
                _ => ScanResult {
                    scanner_name: format!("Unknown ({})", id),
                    findings: Vec::new(),
                    total_size: 0,
                    duration: 0,
                },
            }
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(_) => {}
        }
    }

    results
}
