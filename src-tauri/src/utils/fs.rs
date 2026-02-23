//! Filesystem utilities — safe directory reading, size calculation, and file age.
//! All functions swallow errors and return safe defaults (0, empty, false).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Entry from a directory listing, including type info.
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_directory: bool,
}

/// Synchronously calculate directory size by walking with a stack.
pub fn get_size_sync(path: &Path) -> u64 {
    let mut total: u64 = 0;
    let mut stack = vec![path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_file() {
                total += meta.len();
            } else if meta.is_dir() {
                stack.push(entry.path());
            }
        }
    }

    total
}

/// Get the size of a file or directory in bytes.
/// For directories, walks recursively using pure Rust (no subprocess).
pub async fn get_size(path: &Path) -> u64 {
    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return 0,
    };

    if meta.is_file() {
        return meta.len();
    }

    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || get_size_sync(&path))
        .await
        .unwrap_or(0)
}

/// Synchronously get the age of a file/directory in milliseconds.
pub fn get_file_age_sync(path: &Path) -> u64 {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return 0,
    };
    let modified = match meta.modified() {
        Ok(t) => t,
        Err(_) => return 0,
    };
    SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default()
        .as_millis() as u64
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
