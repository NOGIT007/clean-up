//! Clean Up — Interactive macOS cleanup tool.
//! Tauri v2 backend: scanners, utilities, and IPC commands.

pub mod commands;
pub mod scanners;
pub mod types;
pub mod utils;

use commands::AppState;
use tauri::Manager;

/// Build and run the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_scanners,
            commands::get_version,
            commands::run_scan,
            commands::trash_items,
            commands::list_apps,
            commands::get_app_data,
            commands::reindex_spotlight,
            commands::spotlight_status,
            commands::check_permissions,
            commands::open_settings,
            commands::open_trash,
        ])
        .register_asynchronous_uri_scheme_protocol("appicon", |ctx, request, responder| {
            let app = ctx.app_handle().clone();

            tauri::async_runtime::spawn(async move {
                let url = request.uri().to_string();
                // URL format: appicon://localhost/<encoded-app-path>
                let app_path = url
                    .strip_prefix("appicon://localhost/")
                    .or_else(|| url.strip_prefix("appicon:///"))
                    .or_else(|| url.strip_prefix("appicon://"))
                    .unwrap_or("")
                    .to_string();

                let decoded = urlencoding_decode(&app_path);
                let state = app.state::<AppState>();

                match commands::get_app_icon_png(&decoded, &state.icon_cache).await {
                    Some(data) => {
                        let response = tauri::http::Response::builder()
                            .status(200)
                            .header("Content-Type", "image/png")
                            .header("Cache-Control", "public, max-age=3600")
                            .body(data)
                            .unwrap();
                        responder.respond(response);
                    }
                    None => {
                        let response = tauri::http::Response::builder()
                            .status(404)
                            .body(b"No icon".to_vec())
                            .unwrap();
                        responder.respond(response);
                    }
                }
            });
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Simple URL decoding (percent-decode).
fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}
