//! Trash utilities — move files to macOS Trash (recoverable). Never uses rm.
//! Includes path safety blocklist to prevent catastrophic deletions.

use crate::types::TrashResult;
use std::collections::HashSet;
use std::sync::LazyLock;
use tokio::process::Command;

/// Hardcoded blocklist of paths that must NEVER be trashed.
/// These are critical system directories.
static BLOCKED_PATHS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "/",
        "/System",
        "/System/Library",
        "/Library",
        "/Users",
        "/Applications",
        "/bin",
        "/sbin",
        "/usr",
        "/usr/bin",
        "/usr/lib",
        "/usr/local",
        "/usr/sbin",
        "/etc",
        "/var",
        "/tmp",
        "/private",
        "/private/etc",
        "/private/var",
        "/private/tmp",
        "/opt",
        "/opt/homebrew",
        "/Volumes",
        "/cores",
        "/dev",
        "/net",
        "/home",
    ]
    .into_iter()
    .collect()
});

/// Patterns that should never be trashed.
/// Checked as prefixes against the normalized path.
const BLOCKED_PREFIXES: &[&str] = &[
    "/System/",
    "/usr/",
    "/bin/",
    "/sbin/",
    "/private/etc/",
    "/private/var/",
];

/// Check if a path is safe to trash.
/// Returns true if the path is NOT in the blocklist.
pub fn is_path_safe(path: &str) -> bool {
    // Normalize: remove trailing slashes
    let normalized = path.trim_end_matches('/');
    let normalized = if normalized.is_empty() {
        "/"
    } else {
        normalized
    };

    // Direct blocklist check
    if BLOCKED_PATHS.contains(normalized) {
        return false;
    }

    // Prefix check
    for prefix in BLOCKED_PREFIXES {
        if normalized.starts_with(prefix) {
            return false;
        }
    }

    // Home directory itself is blocked
    if let Ok(home) = std::env::var("HOME") {
        let home_normalized = home.trim_end_matches('/');
        if normalized == home_normalized {
            return false;
        }
    }

    true
}

/// Move a single file or directory to macOS Trash.
/// Tries the `trash` CLI first, falls back to AppleScript via osascript.
///
/// Returns true if successfully trashed, false otherwise.
pub async fn move_to_trash(path: &str) -> bool {
    // Safety check
    if !is_path_safe(path) {
        eprintln!("BLOCKED: Refusing to trash protected path: {path}");
        return false;
    }

    // Try the `trash` CLI first (homebrew: brew install trash)
    if let Ok(output) = Command::new("trash").arg(path).output().await {
        if output.status.success() {
            return true;
        }
    }

    // Fallback: AppleScript via osascript
    let escaped = path.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        "tell application \"Finder\" to delete POSIX file \"{}\"",
        escaped
    );

    match Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Move multiple files/directories to macOS Trash in a single operation.
/// Batches into one process to avoid multiple credential prompts.
/// Falls back to individual `move_to_trash()` if the batch fails.
pub async fn move_multiple_to_trash(paths: &[String]) -> Vec<TrashResult> {
    let safe_paths: Vec<&String> = paths
        .iter()
        .filter(|p| {
            if !is_path_safe(p) {
                eprintln!("BLOCKED: Refusing to trash protected path: {p}");
                false
            } else {
                true
            }
        })
        .collect();

    if safe_paths.is_empty() {
        return paths
            .iter()
            .map(|p| TrashResult {
                path: p.clone(),
                success: false,
            })
            .collect();
    }

    // Try the `trash` CLI with all paths as args (single process)
    let mut cmd = Command::new("trash");
    for p in &safe_paths {
        cmd.arg(p.as_str());
    }
    if let Ok(output) = cmd.output().await {
        if output.status.success() {
            return paths
                .iter()
                .map(|p| TrashResult {
                    path: p.clone(),
                    success: safe_paths.iter().any(|sp| sp.as_str() == p.as_str()),
                })
                .collect();
        }
    }

    // Fallback: single AppleScript with all paths
    let posix_files: Vec<String> = safe_paths
        .iter()
        .map(|p| {
            let escaped = p.replace('\\', "\\\\").replace('"', "\\\"");
            format!("POSIX file \"{}\"", escaped)
        })
        .collect();
    let script = format!(
        "tell application \"Finder\" to delete {{{}}}",
        posix_files.join(", ")
    );

    if let Ok(output) = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .await
    {
        if output.status.success() {
            return paths
                .iter()
                .map(|p| TrashResult {
                    path: p.clone(),
                    success: safe_paths.iter().any(|sp| sp.as_str() == p.as_str()),
                })
                .collect();
        }
    }

    // Final fallback: individual move_to_trash for granular results
    let mut results = Vec::with_capacity(paths.len());
    for p in paths {
        let success = move_to_trash(p).await;
        results.push(TrashResult {
            path: p.clone(),
            success,
        });
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_system_paths() {
        assert!(!is_path_safe("/"));
        assert!(!is_path_safe("/System"));
        assert!(!is_path_safe("/System/Library"));
        assert!(!is_path_safe("/Library"));
        assert!(!is_path_safe("/Users"));
        assert!(!is_path_safe("/Applications"));
        assert!(!is_path_safe("/usr"));
        assert!(!is_path_safe("/usr/bin"));
        assert!(!is_path_safe("/usr/local"));
        assert!(!is_path_safe("/bin"));
        assert!(!is_path_safe("/sbin"));
        assert!(!is_path_safe("/private"));
        assert!(!is_path_safe("/opt/homebrew"));
    }

    #[test]
    fn blocked_prefixes() {
        assert!(!is_path_safe("/System/Library/Frameworks"));
        assert!(!is_path_safe("/usr/lib/libSystem.B.dylib"));
        assert!(!is_path_safe("/bin/sh"));
        assert!(!is_path_safe("/sbin/mount"));
        assert!(!is_path_safe("/private/etc/hosts"));
        assert!(!is_path_safe("/private/var/db"));
    }

    #[test]
    fn trailing_slash_normalized() {
        assert!(!is_path_safe("/System/"));
        assert!(!is_path_safe("/usr/"));
        assert!(!is_path_safe("///"));
    }

    #[test]
    fn home_directory_blocked() {
        // This test depends on HOME env var being set (always true on macOS)
        if let Ok(home) = std::env::var("HOME") {
            assert!(!is_path_safe(&home));
            assert!(!is_path_safe(&format!("{}/", home)));
        }
    }

    #[test]
    fn safe_paths_allowed() {
        assert!(is_path_safe("/Users/test/Library/Caches/com.example"));
        assert!(is_path_safe("/Applications/SomeApp.app"));
        assert!(is_path_safe("/tmp/test-file")); // /tmp itself is blocked but children are ok...
        // Actually /tmp is in BLOCKED_PATHS but /tmp/test-file doesn't match
        // any prefix, so it should be safe.
    }

    #[test]
    fn user_library_subpaths_safe() {
        // These are the kinds of paths scanners will find
        assert!(is_path_safe(
            "/Users/test/Library/Application Support/com.example"
        ));
        assert!(is_path_safe("/Users/test/Library/Caches/com.brave.Browser"));
        assert!(is_path_safe("/Users/test/code/project/node_modules"));
    }
}
