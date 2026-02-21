//! Filesystem utilities — safe directory reading, size calculation, and file age.
//! All functions swallow errors and return safe defaults (0, empty, false).

use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::process::Command;

/// Entry from a directory listing, including type info.
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_directory: bool,
}

/// Get the size of a file or directory in bytes.
/// For directories, shells out to `du -sk` for accuracy.
pub async fn get_size(path: &Path) -> u64 {
    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return 0,
    };

    if meta.is_file() {
        return meta.len();
    }

    // For directories, use du -sk (size in KB)
    match Command::new("du")
        .args(["-sk"])
        .arg(path)
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let kb_str = text.trim().split('\t').next().unwrap_or("0");
            kb_str.parse::<u64>().unwrap_or(0) * 1024
        }
        _ => 0,
    }
}

/// Get the age of a file/directory in milliseconds since last modification.
pub async fn get_file_age(path: &Path) -> u64 {
    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return 0,
    };

    let modified = match meta.modified() {
        Ok(t) => t,
        Err(_) => return 0,
    };

    let elapsed = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default();

    elapsed.as_millis() as u64
}

/// Safely read a directory's contents.
/// Returns empty vec on permission errors or if path doesn't exist.
pub async fn safe_readdir(path: &Path) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    let mut dir = match tokio::fs::read_dir(path).await {
        Ok(d) => d,
        Err(_) => return entries,
    };

    loop {
        match dir.next_entry().await {
            Ok(Some(entry)) => entries.push(entry.path()),
            Ok(None) => break,
            Err(_) => break,
        }
    }

    entries
}

/// Safely read directory entries with type information.
/// Returns empty vec on error.
pub async fn safe_readdir_with_types(path: &Path) -> Vec<DirEntry> {
    let mut entries = Vec::new();
    let mut dir = match tokio::fs::read_dir(path).await {
        Ok(d) => d,
        Err(_) => return entries,
    };

    loop {
        match dir.next_entry().await {
            Ok(Some(entry)) => {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path();
                let is_directory = entry
                    .file_type()
                    .await
                    .map(|ft| ft.is_dir())
                    .unwrap_or(false);
                entries.push(DirEntry {
                    name,
                    path,
                    is_directory,
                });
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    entries
}

/// Check if a path exists.
pub async fn path_exists(path: &Path) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}

/// Check if a path is a directory.
pub async fn is_directory(path: &Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false)
}
