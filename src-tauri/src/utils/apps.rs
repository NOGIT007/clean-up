//! macOS app detection utilities.
//! Uses mdfind/mdls to find installed app bundle IDs without dependencies.

use crate::types::{AppInfo, AppUninstallData};
use crate::utils::fs::{get_size, safe_readdir};
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::process::Command;

/// Regex for reverse-DNS bundle IDs: com.example.app, org.mozilla.firefox
static BUNDLE_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z][a-zA-Z0-9-]*\.[a-zA-Z][a-zA-Z0-9.-]*$").unwrap()
});

/// Minimum app size to show in the uninstaller list.
const MIN_APP_SIZE: u64 = 1024 * 1024; // 1 MB

/// Batch size for mdls subprocess calls.
const BATCH_SIZE: usize = 50;

/// ~/Library subdirectories to scan for associated app data.
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

/// System/Apple bundle ID prefixes to never flag.
const SYSTEM_PREFIXES: &[&str] = &[
    "com.apple.",
    "com.microsoft.rdc",
    "group.com.apple.",
    "systemgroup.",
];

/// Get a set of currently installed app bundle identifiers.
/// Uses `mdfind` + `mdls` to query Spotlight.
pub async fn get_installed_apps() -> HashSet<String> {
    let mut bundle_ids = HashSet::new();

    let output = match Command::new("mdfind")
        .arg("kMDItemContentTypeTree == com.apple.application-bundle")
        .output()
        .await
    {
        Ok(o) if o.status.success() => o,
        _ => return bundle_ids,
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let app_paths: Vec<&str> = text.trim().split('\n').filter(|s| !s.is_empty()).collect();

    // Process in batches
    for batch in app_paths.chunks(BATCH_SIZE) {
        let mut handles = Vec::new();
        for app_path in batch {
            let app_path = app_path.to_string();
            handles.push(tokio::spawn(async move {
                get_bundle_id_from_mdls(&app_path).await
            }));
        }
        for handle in handles {
            if let Ok(Some(id)) = handle.await {
                bundle_ids.insert(id.to_lowercase());
            }
        }
    }

    bundle_ids
}

/// Get bundle ID from mdls for a single app path.
async fn get_bundle_id_from_mdls(app_path: &str) -> Option<String> {
    let output = Command::new("mdls")
        .args(["-name", "kMDItemCFBundleIdentifier", app_path])
        .output()
        .await
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    extract_quoted_value(&text)
}

/// Extract a quoted string value from mdls output.
/// Matches: kMDItemSomething = "value"
fn extract_quoted_value(text: &str) -> Option<String> {
    let start = text.find('"')? + 1;
    let end = text[start..].find('"')? + start;
    Some(text[start..end].to_string())
}

/// Extract a likely bundle ID from a directory name.
/// E.g. "com.spotify.client" from a directory named "com.spotify.client"
pub fn extract_bundle_id(dir_name: &str) -> Option<String> {
    if BUNDLE_ID_RE.is_match(dir_name) {
        Some(dir_name.to_lowercase())
    } else {
        None
    }
}

/// Check if a bundle ID belongs to a system/Apple service.
/// These should never be flagged as orphans.
pub fn is_system_bundle_id(bundle_id: &str) -> bool {
    let lower = bundle_id.to_lowercase();
    SYSTEM_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

/// Get a list of installed non-system apps with display name, path,
/// bundle ID, and size. Filters out system apps and tiny bundles.
pub async fn get_installed_apps_list() -> Vec<AppInfo> {
    let output = match Command::new("mdfind")
        .arg("kMDItemContentTypeTree == com.apple.application-bundle")
        .output()
        .await
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let app_paths: Vec<String> = text
        .trim()
        .split('\n')
        .filter(|p| {
            !p.is_empty()
                && !p.starts_with("/System/")
                && !p.contains("/Library/Apple/")
                && p.ends_with(".app")
        })
        .map(String::from)
        .collect();

    let mut apps = Vec::new();

    for batch in app_paths.chunks(BATCH_SIZE) {
        let mut handles = Vec::new();
        for app_path in batch {
            let app_path = app_path.clone();
            handles.push(tokio::spawn(async move {
                get_app_info(&app_path).await
            }));
        }
        for handle in handles {
            if let Ok(Some(info)) = handle.await {
                apps.push(info);
            }
        }
    }

    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps
}

/// Get app info for a single .app path via mdls.
async fn get_app_info(app_path: &str) -> Option<AppInfo> {
    let output = Command::new("mdls")
        .args([
            "-name",
            "kMDItemCFBundleIdentifier",
            "-name",
            "kMDItemDisplayName",
            app_path,
        ])
        .output()
        .await
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);

    // Parse bundle ID
    let bundle_id = {
        let re = Regex::new(r#"kMDItemCFBundleIdentifier\s*=\s*"([^"]+)""#).ok()?;
        let caps = re.captures(&text)?;
        caps.get(1)?.as_str().to_string()
    };

    if is_system_bundle_id(&bundle_id) {
        return None;
    }

    // Parse display name
    let display_name = {
        let re = Regex::new(r#"kMDItemDisplayName\s*=\s*"([^"]+)""#).ok();
        re.and_then(|r| r.captures(&text))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| {
                Path::new(app_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .trim_end_matches(".app")
                    .to_string()
            })
    };

    let app_size = get_size(Path::new(app_path)).await;
    if app_size < MIN_APP_SIZE {
        return None;
    }

    Some(AppInfo {
        name: display_name,
        path: app_path.to_string(),
        bundle_id,
        app_size,
    })
}

/// Find all associated data in ~/Library for a given app.
/// Matches by bundle ID, app name, and plist files.
pub async fn get_app_associated_data(
    bundle_id: &str,
    app_name: &str,
) -> Vec<AppUninstallData> {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    let mut results = Vec::new();
    let id_lower = bundle_id.to_lowercase();
    let name_lower = app_name.to_lowercase().replace(".app", "");

    for subdir in LIBRARY_SUBDIRS {
        let dir_path = PathBuf::from(&home).join("Library").join(subdir);
        let entries = safe_readdir(&dir_path).await;

        for entry_path in &entries {
            let name = match entry_path.file_name() {
                Some(n) => n.to_string_lossy().to_string(),
                None => continue,
            };
            let lower = name.to_lowercase();

            // Match by bundle ID (exact, case-insensitive)
            let is_bundle_match = lower == id_lower;
            // Match by app name
            let is_name_match =
                lower == name_lower || lower == format!("{}.app", name_lower);
            // Match plist by bundle ID in Preferences
            let is_plist_match = *subdir == "Preferences"
                && (lower == format!("{}.plist", id_lower)
                    || lower == format!("{}.savedstate", id_lower));

            if !is_bundle_match && !is_name_match && !is_plist_match {
                continue;
            }

            let size = get_size(entry_path).await;
            results.push(AppUninstallData {
                path: entry_path.to_string_lossy().to_string(),
                label: format!("{}/{}", subdir, name),
                size,
            });
        }
    }

    results.sort_by(|a, b| b.size.cmp(&a.size));
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_bundle_id_valid() {
        assert_eq!(
            extract_bundle_id("com.spotify.client"),
            Some("com.spotify.client".to_string())
        );
        assert_eq!(
            extract_bundle_id("org.mozilla.firefox"),
            Some("org.mozilla.firefox".to_string())
        );
        assert_eq!(
            extract_bundle_id("com.apple.Safari"),
            Some("com.apple.safari".to_string())
        );
    }

    #[test]
    fn extract_bundle_id_invalid() {
        assert_eq!(extract_bundle_id(".hidden"), None);
        assert_eq!(extract_bundle_id("no-dots"), None);
        assert_eq!(extract_bundle_id(""), None);
        assert_eq!(extract_bundle_id("123.starts.with.number"), None);
    }

    #[test]
    fn system_bundle_ids_detected() {
        assert!(is_system_bundle_id("com.apple.Safari"));
        assert!(is_system_bundle_id("com.apple.finder"));
        assert!(is_system_bundle_id("com.microsoft.rdc"));
        assert!(is_system_bundle_id("group.com.apple.notes"));
        assert!(is_system_bundle_id("systemgroup.com.apple.something"));
    }

    #[test]
    fn third_party_bundle_ids_not_system() {
        assert!(!is_system_bundle_id("com.spotify.client"));
        assert!(!is_system_bundle_id("org.mozilla.firefox"));
        assert!(!is_system_bundle_id("com.google.Chrome"));
    }

    #[test]
    fn extract_quoted_value_works() {
        assert_eq!(
            extract_quoted_value(r#"kMDItemCFBundleIdentifier = "com.example.app""#),
            Some("com.example.app".to_string())
        );
        assert_eq!(
            extract_quoted_value("kMDItemCFBundleIdentifier = (null)"),
            None
        );
    }
}
