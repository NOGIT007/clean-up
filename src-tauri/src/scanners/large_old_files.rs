//! Large/old files scanner.
//! Walks the home directory (limited depth) to find files that are
//! either very large (>100MB) or very old (>365 days since modification).

use crate::types::Finding;
use crate::types::ScanResult;
use crate::utils::fs::safe_readdir_with_types;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Instant, SystemTime};

/// Max depth to walk from home directory.
const MAX_DEPTH: usize = 5;

/// Size threshold: 100 MB.
const SIZE_THRESHOLD: u64 = 100 * 1024 * 1024;

/// Age threshold: 365 days in ms.
const AGE_THRESHOLD: u64 = 365 * 24 * 60 * 60 * 1000;

/// Maximum number of findings to return.
const MAX_FINDINGS: usize = 50;

/// Directories to skip entirely when walking.
static SKIP_DIRS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "Library",
        ".Trash",
        ".git",
        "node_modules",
        ".next",
        ".venv",
        "venv",
        "__pycache__",
        ".cache",
        ".local",
        ".config",
        ".npm",
        ".bun",
        ".nvm",
        ".volta",
        ".rustup",
        ".cargo",
        ".docker",
        ".orbstack",
        "target",
        "dist",
        "build",
        ".gradle",
        "Applications",
        "Photos Library.photoslibrary",
        "Music",
        "Movies",
        ".Spotlight-V100",
        ".fseventsd",
    ]
    .into_iter()
    .collect()
});

/// Check if a path is inside a git repository (has .git ancestor).
async fn is_inside_git_repo(dir_path: &Path, home: &str) -> bool {
    let mut current = dir_path.to_path_buf();
    let home_len = home.len();

    while current.to_string_lossy().len() >= home_len && current != Path::new("/") {
        if tokio::fs::metadata(current.join(".git")).await.is_ok() {
            return true;
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => break,
        }
    }

    false
}

/// Create and run the large/old files scanner.
pub async fn scan() -> ScanResult {
    let start = Instant::now();
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            return ScanResult {
                scanner_name: "Large & Old Files".to_string(),
                findings: Vec::new(),
                total_size: 0,
                duration: 0,
            };
        }
    };

    let mut findings = Vec::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((PathBuf::from(&home), 0));

    while let Some((dir_path, depth)) = queue.pop_front() {
        if findings.len() >= MAX_FINDINGS {
            break;
        }
        if depth > MAX_DEPTH {
            continue;
        }

        let entries = safe_readdir_with_types(&dir_path).await;

        for entry in entries {
            if findings.len() >= MAX_FINDINGS {
                break;
            }

            // Skip hidden dirs and known skip dirs
            if entry.name.starts_with('.') && entry.is_directory {
                if SKIP_DIRS.contains(entry.name.as_str()) {
                    continue;
                }
                // Skip all hidden dirs at depth 0 (home level)
                if depth == 0 {
                    continue;
                }
            }

            if SKIP_DIRS.contains(entry.name.as_str()) {
                continue;
            }

            if entry.is_directory {
                queue.push_back((entry.path, depth + 1));
                continue;
            }

            // It's a file -- check size and age
            let meta = match tokio::fs::metadata(&entry.path).await {
                Ok(m) => m,
                Err(_) => continue,
            };

            let size = meta.len();
            let age = meta
                .modified()
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            let is_large = size >= SIZE_THRESHOLD;
            let is_old = age >= AGE_THRESHOLD;

            if !is_large && !is_old {
                continue;
            }

            // Files inside git repos must be BOTH large AND old
            let parent_dir = entry.path.parent().unwrap_or(Path::new("/"));
            let in_repo = is_inside_git_repo(parent_dir, &home).await;
            if in_repo && !(is_large && is_old) {
                continue;
            }

            let mut reasons = Vec::new();
            if is_large {
                reasons.push(format!(
                    "Large file (>{}MB)",
                    SIZE_THRESHOLD / 1024 / 1024
                ));
            }
            if is_old {
                reasons.push("Not modified in over a year".to_string());
            }

            let display_path = entry.path.to_string_lossy().replace(&home, "~");

            findings.push(Finding {
                path: entry.path.to_string_lossy().to_string(),
                label: display_path,
                size,
                age,
                reason: reasons.join(" + "),
                effort: None,
            });
        }
    }

    // Sort by size descending
    findings.sort_by(|a, b| b.size.cmp(&a.size));

    let total_size = findings.iter().map(|f| f.size).sum();

    ScanResult {
        scanner_name: "Large & Old Files".to_string(),
        findings,
        total_size,
        duration: start.elapsed().as_millis() as u64,
    }
}
