//! Core type definitions for the Clean Up app.
//! Mirrors the TypeScript types in `src/types.ts` for frontend compatibility.

use serde::{Deserialize, Serialize};

/// Deletion impact level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Effort {
    /// Auto-regenerates (caches, build artifacts).
    None,
    /// Manual action needed (reinstall dependencies, etc.).
    Reinstall,
}

/// A single item found during scanning that could be cleaned up.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Absolute path to the file or directory.
    pub path: String,
    /// Human-readable description of what this is.
    pub label: String,
    /// Size in bytes.
    pub size: u64,
    /// Age in milliseconds since last modification.
    pub age: u64,
    /// Why this was flagged for cleanup.
    pub reason: String,
    /// Deletion impact: "none" = auto-regenerates, "reinstall" = manual action needed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
}

/// Result returned by a scanner after it finishes scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    /// Name of the scanner that produced these results.
    pub scanner_name: String,
    /// List of findings.
    pub findings: Vec<Finding>,
    /// Total size of all findings in bytes.
    pub total_size: u64,
    /// How long the scan took in milliseconds.
    pub duration: u64,
}

/// Metadata about a scanner (sent to frontend for UI rendering).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerInfo {
    /// Unique identifier for this scanner.
    pub id: String,
    /// Human-readable name shown in the UI.
    pub name: String,
    /// Short description of what this scanner finds.
    pub description: String,
}

/// Info about an installed macOS application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub path: String,
    pub bundle_id: String,
    pub app_size: u64,
}

/// A piece of associated data found for an app in ~/Library.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUninstallData {
    pub path: String,
    pub label: String,
    pub size: u64,
}

/// Result from a single trash operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashResult {
    pub path: String,
    pub success: bool,
}
