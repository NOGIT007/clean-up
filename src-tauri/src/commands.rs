//! Tauri IPC commands — replaces all HTTP endpoints from the web server.
//! Each command is invoked from the frontend via `window.__TAURI__.invoke()`.

use crate::types::{AppInfo, AppUninstallData, ScanResult, ScannerInfo, TrashResult};
use crate::utils::apps;
use crate::utils::trash;
use crate::scanners;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tokio::process::Command;

/// Shared app state managed by Tauri.
pub struct AppState {
    /// In-memory cache for app icon PNGs.
    pub icon_cache: Mutex<HashMap<String, Option<Vec<u8>>>>,
    /// Timestamp of last Spotlight reindex request (ms since epoch).
    pub last_reindex_time: AtomicU64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            icon_cache: Mutex::new(HashMap::new()),
            last_reindex_time: AtomicU64::new(0),
        }
    }
}

/// Version info response.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub version: String,
    pub built: String,
}

/// Spotlight status response.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotlightStatus {
    pub indexing: bool,
    pub enabled: bool,
    pub raw: String,
}

/// Permission check result.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub id: String,
    pub name: String,
    pub description: String,
    pub granted: bool,
    pub deep_link: String,
}

/// Trash operation response.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashResponse {
    pub results: Vec<TrashResult>,
}

// ---- Tauri Commands ----

/// List all available scanners with their metadata.
#[tauri::command]
pub fn list_scanners() -> Vec<ScannerInfo> {
    scanners::all_scanner_info()
}

/// Get version information.
#[tauri::command]
pub fn get_version() -> VersionInfo {
    VersionInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        built: option_env!("BUILD_TIME")
            .unwrap_or("dev")
            .to_string(),
    }
}

/// Run selected scanners and return results.
#[tauri::command]
pub async fn run_scan(scanner_ids: Vec<String>) -> Vec<ScanResult> {
    scanners::run_scanners(&scanner_ids).await
}

/// Move items to macOS Trash.
#[tauri::command]
pub async fn trash_items(paths: Vec<String>) -> TrashResponse {
    let results = trash::move_multiple_to_trash(&paths).await;
    TrashResponse { results }
}

/// List installed non-system macOS applications.
#[tauri::command]
pub async fn list_apps() -> Vec<AppInfo> {
    apps::get_installed_apps_list().await
}

/// Get associated ~/Library data for a specific app.
#[tauri::command]
pub async fn get_app_data(
    bundle_id: String,
    app_name: String,
) -> Vec<AppUninstallData> {
    apps::get_app_associated_data(&bundle_id, &app_name).await
}

/// Trigger a Spotlight reindex (requires admin privileges via native dialog).
#[tauri::command]
pub async fn reindex_spotlight(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let script = r#"do shell script "mdutil -E /" with administrator privileges"#;
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        state.last_reindex_time.store(now, Ordering::Relaxed);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains("-128") {
            Err("cancelled".to_string())
        } else {
            Err(stderr)
        }
    }
}

/// Check Spotlight indexing status.
#[tauri::command]
pub async fn spotlight_status(
    state: tauri::State<'_, AppState>,
) -> Result<SpotlightStatus, String> {
    let output = Command::new("mdutil")
        .args(["-s", "/"])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let text_lower = text.to_lowercase();

    let mut indexing = text_lower.contains("indexing") && text_lower.contains("progress");
    let enabled = text_lower.contains("enabled");

    // Grace period: if we recently triggered a reindex, report indexing anyway
    let grace_ms: u64 = 5 * 60 * 1000; // 5 minutes
    let last_reindex = state.last_reindex_time.load(Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    if !indexing && last_reindex > 0 && now.saturating_sub(last_reindex) < grace_ms {
        indexing = true;
    }

    // Clear flag once Spotlight is no longer indexing and grace period is over
    if !indexing && last_reindex > 0 {
        state.last_reindex_time.store(0, Ordering::Relaxed);
    }

    Ok(SpotlightStatus {
        indexing,
        enabled,
        raw: text.trim().to_string(),
    })
}

/// Check macOS privacy permissions.
#[tauri::command]
pub async fn check_permissions() -> Vec<Permission> {
    let home = std::env::var("HOME").unwrap_or_else(|_| {
        format!(
            "/Users/{}",
            std::env::var("USER").unwrap_or_default()
        )
    });

    let (fda, automation, app_mgmt) = tokio::join!(
        // Full Disk Access: try reading ~/Library/Safari
        async {
            tokio::fs::read_dir(format!("{}/Library/Safari", home))
                .await
                .is_ok()
        },
        // Automation (Finder): try osascript
        async {
            Command::new("osascript")
                .args(["-e", r#"tell app "Finder" to get name of home"#])
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false)
        },
        // App Management: writable /Applications
        async {
            // Check write access by attempting to create/remove a temp entry
            tokio::fs::metadata("/Applications")
                .await
                .map(|m| {
                    // On macOS, check if we have write permission
                    // A simple heuristic: if we can read the dir, check permissions
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mode = m.permissions().mode();
                        // Check if user/group/other has write
                        mode & 0o222 != 0
                    }
                    #[cfg(not(unix))]
                    {
                        !m.permissions().readonly()
                    }
                })
                .unwrap_or(false)
        }
    );

    vec![
        Permission {
            id: "full-disk-access".to_string(),
            name: "Full Disk Access".to_string(),
            description: "Required for scanning Safari caches, iCloud data, and protected ~/Library directories".to_string(),
            granted: fda,
            deep_link: "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles".to_string(),
        },
        Permission {
            id: "automation".to_string(),
            name: "Automation (Finder)".to_string(),
            description: "Required for moving files to Trash via Finder \u{2014} the safe deletion method".to_string(),
            granted: automation,
            deep_link: "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation".to_string(),
        },
        Permission {
            id: "app-management".to_string(),
            name: "App Management".to_string(),
            description: "Allows uninstalling apps from /Applications".to_string(),
            granted: app_mgmt,
            deep_link: "x-apple.systempreferences:com.apple.preference.security?Privacy_AppManagement".to_string(),
        },
    ]
}

/// Open System Settings to a specific pane.
#[tauri::command]
pub async fn open_settings(deep_link: String) -> Result<(), String> {
    if !deep_link.starts_with("x-apple.systempreferences:") {
        return Err("Invalid deep link".to_string());
    }

    Command::new("open")
        .arg(&deep_link)
        .output()
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Open Trash in Finder.
#[tauri::command]
pub async fn open_trash() -> Result<(), String> {
    Command::new("open")
        .arg("trash://")
        .output()
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Get an app icon as PNG bytes (used by appicon:// protocol).
pub async fn get_app_icon_png(
    app_path: &str,
    cache: &Mutex<HashMap<String, Option<Vec<u8>>>>,
) -> Option<Vec<u8>> {
    // Check cache
    {
        let cache_guard = cache.lock().ok()?;
        if let Some(cached) = cache_guard.get(app_path) {
            return cached.clone();
        }
    }

    let result = fetch_app_icon(app_path).await;

    // Store in cache
    if let Ok(mut cache_guard) = cache.lock() {
        cache_guard.insert(app_path.to_string(), result.clone());
    }

    result
}

/// Actually fetch and convert an app icon to PNG.
async fn fetch_app_icon(app_path: &str) -> Option<Vec<u8>> {
    let plist_path = format!("{}/Contents/Info.plist", app_path);
    if !crate::utils::fs::path_exists(std::path::Path::new(&plist_path)).await {
        return None;
    }

    // Extract CFBundleIconFile using plutil
    let output = Command::new("plutil")
        .args(["-extract", "CFBundleIconFile", "raw", &plist_path])
        .output()
        .await
        .ok()?;

    let mut icon_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if icon_name.is_empty() {
        return None;
    }

    // Ensure .icns extension
    if !icon_name.ends_with(".icns") {
        icon_name.push_str(".icns");
    }

    let icns_path = format!("{}/Contents/Resources/{}", app_path, icon_name);

    // Read icns file and extract embedded PNG directly (no sips needed)
    let icns_data = tokio::fs::read(&icns_path).await.ok()?;
    extract_png_from_icns(&icns_data)
}

/// Icon types preferred for ~64px display, best to worst.
/// Modern icns files embed PNG data directly for these types.
const PREFERRED_ICON_TYPES: &[[u8; 4]] = &[
    *b"ic12", // 64x64@2x (128x128 Retina)
    *b"ic11", // 32x32@2x (64x64 Retina)
    *b"ic07", // 128x128
    *b"icp5", // 32x32
    *b"ic08", // 256x256
    *b"ic13", // 128x128@2x
    *b"ic09", // 512x512
    *b"ic10", // 1024x1024
    *b"ic14", // 256x256@2x
];

/// Extract an embedded PNG from an .icns file by parsing its binary format.
fn extract_png_from_icns(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 8 || &data[0..4] != b"icns" {
        return None;
    }

    let mut best: Option<(usize, &[u8])> = None; // (priority, data)
    let mut pos = 8;

    while pos + 8 <= data.len() {
        let icon_type: [u8; 4] = [data[pos], data[pos + 1], data[pos + 2], data[pos + 3]];
        let entry_size = u32::from_be_bytes([
            data[pos + 4],
            data[pos + 5],
            data[pos + 6],
            data[pos + 7],
        ]) as usize;

        if entry_size < 8 || pos + entry_size > data.len() {
            break;
        }

        let entry_data = &data[pos + 8..pos + entry_size];

        // Check if entry contains PNG data (PNG magic: 0x89 P N G)
        if entry_data.len() >= 8
            && entry_data[0] == 0x89
            && entry_data[1] == 0x50
            && entry_data[2] == 0x4E
            && entry_data[3] == 0x47
        {
            for (priority, preferred) in PREFERRED_ICON_TYPES.iter().enumerate() {
                if icon_type == *preferred {
                    match &best {
                        Some((best_p, _)) if *best_p <= priority => {}
                        _ => best = Some((priority, entry_data)),
                    }
                    break;
                }
            }
            // Use as fallback if no priority match yet
            if best.is_none() {
                best = Some((usize::MAX, entry_data));
            }
        }

        pos += entry_size;
    }

    best.map(|(_, d)| d.to_vec())
}
