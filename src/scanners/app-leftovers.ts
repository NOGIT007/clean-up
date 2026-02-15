/**
 * App leftovers scanner.
 * Finds orphaned application data in ~/Library for apps that are
 * no longer installed. Skips Apple system services.
 */

import type { Finding, Scanner, ScanResult } from "../types";
import {
  extractBundleId,
  getInstalledApps,
  isSystemBundleId,
} from "../utils/apps";
import { getFileAge, getSize, safeReaddir } from "../utils/fs";
import { basename } from "node:path";

/** ~/Library subdirectories to scan for orphaned app data. */
const LIBRARY_SUBDIRS = [
  "Application Support",
  "Caches",
  "Containers",
  "Logs",
  "Preferences",
  "Saved Application State",
  "WebKit",
  "HTTPStorages",
];

/** Minimum size to report (skip tiny preference files). */
const MIN_SIZE = 512 * 1024; // 512 KB

/**
 * Additional names to always skip (not bundle-ID based but safe).
 */
const ALWAYS_SKIP = new Set([
  ".ds_store",
  ".localized",
  "apple",
  "com.apple.appstore",
]);

export function createAppLeftoversScanner(): Scanner {
  return {
    id: "app-leftovers",
    name: "App Leftovers",
    description: "Orphaned data from uninstalled applications",

    async scan(): Promise<ScanResult> {
      const startTime = Date.now();
      const home = process.env.HOME;
      if (!home) {
        return {
          scannerName: "App Leftovers",
          findings: [],
          totalSize: 0,
          duration: Date.now() - startTime,
        };
      }

      // Get currently installed apps
      const installedApps = await getInstalledApps();
      const findings: Finding[] = [];

      for (const subdir of LIBRARY_SUBDIRS) {
        const dirPath = `${home}/Library/${subdir}`;
        const entries = await safeReaddir(dirPath);

        for (const entryPath of entries) {
          const name = basename(entryPath);

          // Skip hidden files and known safe entries
          if (name.startsWith(".")) continue;
          if (ALWAYS_SKIP.has(name.toLowerCase())) continue;

          // Try to extract a bundle ID
          const bundleId = extractBundleId(name);
          if (!bundleId) continue;

          // Skip Apple/system services
          if (isSystemBundleId(bundleId)) continue;

          // Skip if app is currently installed
          if (installedApps.has(bundleId)) continue;

          // This looks like orphaned data — measure it
          const size = await getSize(entryPath);
          if (size < MIN_SIZE) continue;

          const age = await getFileAge(entryPath);

          findings.push({
            path: entryPath,
            label: `${name} in ~/Library/${subdir}`,
            size,
            age,
            reason: `Data from uninstalled app (${bundleId})`,
            effort: "none",
          });
        }
      }

      // Sort by size descending
      findings.sort((a, b) => b.size - a.size);

      const totalSize = findings.reduce((sum, f) => sum + f.size, 0);

      return {
        scannerName: "App Leftovers",
        findings,
        totalSize,
        duration: Date.now() - startTime,
      };
    },
  };
}
