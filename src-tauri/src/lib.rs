//! Clean Up — Interactive macOS cleanup tool.
//! Tauri v2 backend: scanners, utilities, and IPC commands.

pub mod scanners;
pub mod types;
pub mod utils;

/// Build and run the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
