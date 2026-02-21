//! Homebrew cleanup scanner.
//! Detects old formula versions and stale cache using `brew cleanup --dry-run`.

use crate::types::Finding;
use crate::types::ScanResult;
use crate::utils::fs::{get_size, path_exists};
use regex::Regex;
use std::path::Path;
use std::time::Instant;
use tokio::process::Command;

/// Find the brew binary path, or None if not installed.
async fn find_brew_path() -> Option<String> {
    // Check common locations first (faster than shelling out)
    let known_paths = [
        "/opt/homebrew/bin/brew", // Apple Silicon
        "/usr/local/bin/brew",   // Intel
    ];

    for p in &known_paths {
        if path_exists(Path::new(p)).await {
            return Some(p.to_string());
        }
    }

    // Fallback: which brew
    if let Ok(output) = Command::new("which").arg("brew").output().await {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    None
}

/// Classify a path from brew cleanup output into a human-readable label.
fn classify_path(file_path: &str) -> (String, String) {
    // /Cellar/<name>/<version>
    let cellar_re = Regex::new(r"/Cellar/([^/]+)/([^/]+)").unwrap();
    if let Some(caps) = cellar_re.captures(file_path) {
        let name = caps.get(1).unwrap().as_str();
        let version = caps.get(2).unwrap().as_str();
        return (
            format!("{}@{}", name, version),
            "Old formula version \u{2014} newer version installed".to_string(),
        );
    }

    // /Caskroom/<name>/<version>
    let cask_re = Regex::new(r"/Caskroom/([^/]+)/([^/]+)").unwrap();
    if let Some(caps) = cask_re.captures(file_path) {
        let name = caps.get(1).unwrap().as_str();
        let version = caps.get(2).unwrap().as_str();
        return (
            format!("{}@{}", name, version),
            "Old cask version \u{2014} newer version installed".to_string(),
        );
    }

    // Cache files
    if file_path.contains("Caches/Homebrew") || file_path.contains("cache") {
        let filename = file_path.rsplit('/').next().unwrap_or(file_path);
        return (
            format!("Cached download: {}", filename),
            "Stale Homebrew cache file \u{2014} safe to remove".to_string(),
        );
    }

    // Fallback
    let name = file_path.rsplit('/').next().unwrap_or(file_path);
    (
        name.to_string(),
        "Old Homebrew artifact \u{2014} safe to remove".to_string(),
    )
}

/// Create and run the homebrew cleanup scanner.
pub async fn scan() -> ScanResult {
    let start = Instant::now();

    let brew_path = match find_brew_path().await {
        Some(p) => p,
        None => {
            return ScanResult {
                scanner_name: "Homebrew Cleanup".to_string(),
                findings: Vec::new(),
                total_size: 0,
                duration: start.elapsed().as_millis() as u64,
            };
        }
    };

    // Run brew cleanup --dry-run to list what would be removed
    let output = match Command::new(&brew_path)
        .args(["cleanup", "--dry-run"])
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => {
            return ScanResult {
                scanner_name: "Homebrew Cleanup".to_string(),
                findings: Vec::new(),
                total_size: 0,
                duration: start.elapsed().as_millis() as u64,
            };
        }
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = text.trim().split('\n').filter(|s| !s.is_empty()).collect();

    let path_re = Regex::new(r"(/\S+)").unwrap();
    let mut findings = Vec::new();

    let mut handles = Vec::new();
    for line in lines {
        if let Some(caps) = path_re.captures(line) {
            let file_path = caps.get(1).unwrap().as_str().to_string();
            handles.push(tokio::spawn(async move {
                if !path_exists(Path::new(&file_path)).await {
                    return None;
                }

                let size = get_size(Path::new(&file_path)).await;
                let (label, reason) = classify_path(&file_path);

                Some(Finding {
                    path: file_path,
                    label,
                    size,
                    age: 0, // brew cleanup doesn't give age info
                    reason,
                    effort: None,
                })
            }));
        }
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
        scanner_name: "Homebrew Cleanup".to_string(),
        findings,
        total_size,
        duration: start.elapsed().as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_cellar_path() {
        let (label, reason) = classify_path("/opt/homebrew/Cellar/node/21.0.0");
        assert_eq!(label, "node@21.0.0");
        assert!(reason.contains("Old formula version"));
    }

    #[test]
    fn classify_caskroom_path() {
        let (label, reason) = classify_path("/opt/homebrew/Caskroom/firefox/120.0");
        assert_eq!(label, "firefox@120.0");
        assert!(reason.contains("Old cask version"));
    }

    #[test]
    fn classify_cache_path() {
        let (label, reason) =
            classify_path("/Users/test/Library/Caches/Homebrew/downloads/abc123--node-21.tar.gz");
        assert!(label.starts_with("Cached download:"));
        assert!(reason.contains("Stale Homebrew cache"));
    }

    #[test]
    fn classify_unknown_path() {
        let (label, reason) = classify_path("/opt/homebrew/share/old-thing");
        assert_eq!(label, "old-thing");
        assert!(reason.contains("Old Homebrew artifact"));
    }
}
