/**
 * System caches scanner.
 * Finds browser caches, system logs, dev tool caches, and temp files.
 */

import type { Finding, Scanner, ScanResult } from "../types";
import { getFileAge, getSize, pathExists } from "../utils/fs";

interface CacheTarget {
  /** Absolute path (~ will be expanded). */
  path: string;
  /** Human-readable label. */
  label: string;
  /** Why this is safe to remove. */
  reason: string;
}

function expandHome(path: string): string {
  const home = process.env.HOME ?? "";
  return path.replace(/^~/, home);
}

/** Known cache locations on macOS. */
const CACHE_TARGETS: CacheTarget[] = [
  // Browser caches
  {
    path: "~/Library/Caches/Google/Chrome",
    label: "Chrome cache",
    reason: "Browser cache — Chrome will rebuild it",
  },
  {
    path: "~/Library/Caches/com.google.Chrome",
    label: "Chrome app cache",
    reason: "Browser cache — Chrome will rebuild it",
  },
  {
    path: "~/Library/Caches/Firefox",
    label: "Firefox cache",
    reason: "Browser cache — Firefox will rebuild it",
  },
  {
    path: "~/Library/Caches/com.apple.Safari",
    label: "Safari cache",
    reason: "Browser cache — Safari will rebuild it",
  },
  {
    path: "~/Library/Caches/com.brave.Browser",
    label: "Brave cache",
    reason: "Browser cache — Brave will rebuild it",
  },
  {
    path: "~/Library/Caches/com.microsoft.edgemac",
    label: "Edge cache",
    reason: "Browser cache — Edge will rebuild it",
  },
  {
    path: "~/Library/Caches/com.operasoftware.Opera",
    label: "Opera cache",
    reason: "Browser cache — Opera will rebuild it",
  },
  {
    path: "~/Library/Caches/arc.browser",
    label: "Arc cache",
    reason: "Browser cache — Arc will rebuild it",
  },

  // System logs
  {
    path: "~/Library/Logs",
    label: "User logs",
    reason: "Application logs — usually safe to clear",
  },

  // Dev tool caches
  {
    path: "~/Library/Caches/com.apple.dt.Xcode",
    label: "Xcode cache",
    reason: "Xcode derived data cache — will be rebuilt",
  },
  {
    path: "~/Library/Developer/Xcode/DerivedData",
    label: "Xcode DerivedData",
    reason: "Xcode build artifacts — will be rebuilt on next build",
  },
  {
    path: "~/Library/Developer/Xcode/Archives",
    label: "Xcode Archives",
    reason: "Old Xcode build archives — usually safe to remove",
  },
  {
    path: "~/Library/Developer/CoreSimulator/Caches",
    label: "iOS Simulator caches",
    reason: "Simulator caches — will be rebuilt",
  },
  {
    path: "~/Library/Caches/CocoaPods",
    label: "CocoaPods cache",
    reason: "Pod cache — will be re-downloaded when needed",
  },
  {
    path: "~/Library/Caches/org.carthage.CarthageKit",
    label: "Carthage cache",
    reason: "Carthage dependency cache — will be re-downloaded",
  },
  {
    path: "~/Library/Caches/Homebrew",
    label: "Homebrew cache",
    reason: "Downloaded package archives — safe to remove",
  },
  {
    path: "~/Library/Caches/pip",
    label: "pip cache",
    reason: "Python package cache — will be re-downloaded",
  },
  {
    path: "~/Library/Caches/yarn",
    label: "Yarn cache",
    reason: "Yarn package cache — will be re-downloaded",
  },
  {
    path: "~/.npm/_cacache",
    label: "npm cache",
    reason: "npm package cache — will be re-downloaded",
  },
  {
    path: "~/.bun/install/cache",
    label: "Bun install cache",
    reason: "Bun package cache — will be re-downloaded",
  },

  // Temp files
  {
    path: "~/Library/Caches/com.apple.bird",
    label: "iCloud temp cache",
    reason: "iCloud sync temp data — will be regenerated",
  },
  {
    path: "~/Library/Caches/CloudKit",
    label: "CloudKit cache",
    reason: "CloudKit sync cache — will be regenerated",
  },
];

/** Paths that require macOS Full Disk Access (TCC-protected). */
const TCC_PROTECTED_PATHS = new Set([
  "~/Library/Caches/com.apple.Safari",
  "~/Library/Caches/com.apple.bird",
  "~/Library/Caches/CloudKit",
]);

/** Minimum size to report. */
const MIN_SIZE = 5 * 1024 * 1024; // 5 MB

export function createSystemCachesScanner(): Scanner {
  return {
    id: "system-caches",
    name: "System Caches",
    description: "Browser caches, system logs, dev tool caches",

    async scan(): Promise<ScanResult> {
      const startTime = Date.now();
      const findings: Finding[] = [];

      const checks = CACHE_TARGETS.map(async (target) => {
        const fullPath = expandHome(target.path);
        const exists = await pathExists(fullPath);
        if (!exists) return;

        const size = await getSize(fullPath);
        if (size < MIN_SIZE) return;

        const age = await getFileAge(fullPath);

        const reason = TCC_PROTECTED_PATHS.has(target.path)
          ? target.reason + " (may need Full Disk Access)"
          : target.reason;

        findings.push({
          path: fullPath,
          label: target.label,
          size,
          age,
          reason,
          effort: "none",
        });
      });

      await Promise.all(checks);

      // Sort by size descending
      findings.sort((a, b) => b.size - a.size);

      const totalSize = findings.reduce((sum, f) => sum + f.size, 0);

      return {
        scannerName: "System Caches",
        findings,
        totalSize,
        duration: Date.now() - startTime,
      };
    },
  };
}
