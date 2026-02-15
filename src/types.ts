/**
 * Core type definitions for the clean-up CLI tool.
 */

/** A single item found during scanning that could be cleaned up. */
export interface Finding {
  /** Absolute path to the file or directory */
  path: string;
  /** Human-readable description of what this is */
  label: string;
  /** Size in bytes */
  size: number;
  /** Age in milliseconds since last modification */
  age: number;
  /** Why this was flagged for cleanup */
  reason: string;
  /** Deletion impact: "none" = auto-regenerates, "reinstall" = manual action needed */
  effort?: "none" | "reinstall";
}

/** Result returned by a scanner after it finishes scanning. */
export interface ScanResult {
  /** Name of the scanner that produced these results */
  scannerName: string;
  /** List of findings */
  findings: Finding[];
  /** Total size of all findings in bytes */
  totalSize: number;
  /** How long the scan took in milliseconds */
  duration: number;
}

/** A scanner that looks for cleanable items in a specific category. */
export interface Scanner {
  /** Unique identifier for this scanner */
  id: string;
  /** Human-readable name shown in the TUI */
  name: string;
  /** Short description of what this scanner finds */
  description: string;
  /** Run the scan and return results */
  scan(): Promise<ScanResult>;
}

/** Info about an installed macOS application. */
export interface AppInfo {
  name: string;
  path: string;
  bundleId: string;
  appSize: number;
}

/** A piece of associated data found for an app in ~/Library. */
export interface AppUninstallData {
  path: string;
  label: string;
  size: number;
}

/** Options parsed from CLI arguments. */
export interface CliOptions {
  /** Show help text and exit */
  help: boolean;
  /** Show version and exit */
  version: boolean;
  /** Preview what would be cleaned without actually trashing */
  dryRun: boolean;
  /** Launch local web UI instead of terminal TUI */
  web: boolean;
}
