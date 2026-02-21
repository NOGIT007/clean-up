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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_serializes_as_camel_case() {
        let json = serde_json::to_string(&Effort::None).unwrap();
        assert_eq!(json, r#""none""#);
        let json = serde_json::to_string(&Effort::Reinstall).unwrap();
        assert_eq!(json, r#""reinstall""#);
    }

    #[test]
    fn effort_deserializes_from_camel_case() {
        let e: Effort = serde_json::from_str(r#""none""#).unwrap();
        assert_eq!(e, Effort::None);
        let e: Effort = serde_json::from_str(r#""reinstall""#).unwrap();
        assert_eq!(e, Effort::Reinstall);
    }

    #[test]
    fn finding_serializes_camel_case_fields() {
        let finding = Finding {
            path: "/test".to_string(),
            label: "Test".to_string(),
            size: 1024,
            age: 5000,
            reason: "test reason".to_string(),
            effort: Some(Effort::None),
        };
        let json = serde_json::to_string(&finding).unwrap();
        // Verify camelCase field names
        assert!(json.contains(r#""path""#));
        assert!(json.contains(r#""label""#));
        assert!(json.contains(r#""size""#));
        assert!(json.contains(r#""age""#));
        assert!(json.contains(r#""reason""#));
        assert!(json.contains(r#""effort""#));
    }

    #[test]
    fn finding_effort_none_skipped_when_none() {
        let finding = Finding {
            path: "/test".to_string(),
            label: "Test".to_string(),
            size: 0,
            age: 0,
            reason: "test".to_string(),
            effort: None,
        };
        let json = serde_json::to_string(&finding).unwrap();
        assert!(!json.contains("effort"));
    }

    #[test]
    fn scan_result_camel_case() {
        let result = ScanResult {
            scanner_name: "Test Scanner".to_string(),
            findings: Vec::new(),
            total_size: 42,
            duration: 100,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains(r#""scannerName""#));
        assert!(json.contains(r#""totalSize""#));
    }

    #[test]
    fn app_info_camel_case() {
        let info = AppInfo {
            name: "Test".to_string(),
            path: "/test".to_string(),
            bundle_id: "com.test".to_string(),
            app_size: 1024,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains(r#""bundleId""#));
        assert!(json.contains(r#""appSize""#));
    }
}
