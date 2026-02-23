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
/// Uses a cache to avoid repeated ancestor walks.
fn is_inside_git_repo_cached(
    dir_path: &Path,
    home: &str,
    cache: &mut std::collections::HashMap<PathBuf, bool>,
) -> bool {
    if let Some(&cached) = cache.get(dir_path) {
        return cached;
    }

    let mut current = dir_path.to_path_buf();
    let home_len = home.len();

    while current.to_string_lossy().len() >= home_len && current != Path::new("/") {
        if let Some(&cached) = cache.get(&current) {
            cache.insert(dir_path.to_path_buf(), cached);
            return cached;
        }
        if current.join(".git").exists() {
            cache.insert(current, true);
            cache.insert(dir_path.to_path_buf(), true);
            return true;
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => break,
        }
    }

    cache.insert(dir_path.to_path_buf(), false);
    false
}

/// Walk a single subtree for large/old files (runs in spawn_blocking).
fn walk_subtree_sync(
    start_dir: PathBuf,
    start_depth: usize,
    home: String,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut git_cache = std::collections::HashMap::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((start_dir, start_depth));

    while let Some((dir_path, depth)) = queue.pop_front() {
        if depth > MAX_DEPTH {
            continue;
        }

        let entries = match std::fs::read_dir(&dir_path) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let is_dir = meta.is_dir();

            // Skip hidden dirs and known skip dirs
            if name.starts_with('.') && is_dir {
                if SKIP_DIRS.contains(name.as_str()) {
                    continue;
                }
                continue; // skip all hidden dirs in subtrees
            }

            if SKIP_DIRS.contains(name.as_str()) {
                continue;
            }

            if is_dir {
                queue.push_back((path, depth + 1));
                continue;
            }

            // It's a file -- check size and age
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

            let parent_dir = path.parent().unwrap_or(Path::new("/"));
            let in_repo = is_inside_git_repo_cached(parent_dir, &home, &mut git_cache);
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

            let display_path = path.to_string_lossy().replace(&home, "~");

            findings.push(Finding {
                path: path.to_string_lossy().to_string(),
                label: display_path,
                size,
                age,
                reason: reasons.join(" + "),
                effort: None,
            });
        }
    }

    findings
}

/// Create and run the large/old files scanner.
/// Fans out one task per top-level subdirectory for parallelism.
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

    let top_entries = safe_readdir_with_types(Path::new(&home)).await;

    let mut handles = Vec::new();
    for entry in top_entries {
        if !entry.is_directory {
            continue;
        }
        if SKIP_DIRS.contains(entry.name.as_str()) {
            continue;
        }
        if entry.name.starts_with('.') {
            continue;
        }

        let home = home.clone();
        handles.push(tokio::task::spawn_blocking(move || {
            walk_subtree_sync(entry.path, 1, home)
        }));
    }

    let mut findings = Vec::new();
    for handle in handles {
        if let Ok(subtree_findings) = handle.await {
            findings.extend(subtree_findings);
        }
    }

    // Sort by size descending, cap at MAX_FINDINGS
    findings.sort_by(|a, b| b.size.cmp(&a.size));
    findings.truncate(MAX_FINDINGS);

    let total_size = findings.iter().map(|f| f.size).sum();

    ScanResult {
        scanner_name: "Large & Old Files".to_string(),
        findings,
        total_size,
        duration: start.elapsed().as_millis() as u64,
    }
}
