/**
 * Homebrew cleanup scanner.
 * Detects old formula versions and stale cache using `brew cleanup --dry-run`.
 */

import type { Finding, Scanner, ScanResult } from "../types";
import { getSize, pathExists } from "../utils/fs";

/** Find the brew binary path, or null if not installed. */
async function findBrewPath(): Promise<string | null> {
  // Check common locations first (faster than shelling out)
  const knownPaths = [
    "/opt/homebrew/bin/brew", // Apple Silicon
    "/usr/local/bin/brew", // Intel
  ];

  for (const p of knownPaths) {
    if (await pathExists(p)) return p;
  }

  // Fallback: which brew
  try {
    const proc = Bun.spawn(["which", "brew"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const output = await new Response(proc.stdout).text();
    const exitCode = await proc.exited;
    if (exitCode === 0) {
      const path = output.trim();
      if (path) return path;
    }
  } catch {
    // brew not found
  }

  return null;
}

/**
 * Classify a path from brew cleanup output into a human-readable label.
 * Examples:
 *   /opt/homebrew/Cellar/node/20.0.0 → "node@20.0.0 — Old formula version"
 *   /opt/homebrew/Caskroom/firefox/120.0 → "firefox@120.0 — Old cask version"
 *   ~/Library/Caches/Homebrew/downloads/... → "Cached download: filename"
 */
function classifyPath(filePath: string): { label: string; reason: string } {
  // /Cellar/<name>/<version>
  const cellarMatch = filePath.match(/\/Cellar\/([^/]+)\/([^/]+)/);
  if (cellarMatch) {
    return {
      label: `${cellarMatch[1]}@${cellarMatch[2]}`,
      reason: "Old formula version — newer version installed",
    };
  }

  // /Caskroom/<name>/<version>
  const caskMatch = filePath.match(/\/Caskroom\/([^/]+)\/([^/]+)/);
  if (caskMatch) {
    return {
      label: `${caskMatch[1]}@${caskMatch[2]}`,
      reason: "Old cask version — newer version installed",
    };
  }

  // Cache files
  if (filePath.includes("Caches/Homebrew") || filePath.includes("cache")) {
    const filename = filePath.split("/").pop() ?? filePath;
    return {
      label: `Cached download: ${filename}`,
      reason: "Stale Homebrew cache file — safe to remove",
    };
  }

  // Fallback
  const name = filePath.split("/").pop() ?? filePath;
  return {
    label: name,
    reason: "Old Homebrew artifact — safe to remove",
  };
}

export function createHomebrewCleanupScanner(): Scanner {
  return {
    id: "homebrew-cleanup",
    name: "Homebrew Cleanup",
    description: "Old formula versions and stale Homebrew cache",

    async scan(): Promise<ScanResult> {
      const startTime = Date.now();

      const brewPath = await findBrewPath();
      if (!brewPath) {
        return {
          scannerName: "Homebrew Cleanup",
          findings: [],
          totalSize: 0,
          duration: Date.now() - startTime,
        };
      }

      // Run brew cleanup --dry-run to list what would be removed
      let output: string;
      try {
        const proc = Bun.spawn([brewPath, "cleanup", "--dry-run"], {
          stdout: "pipe",
          stderr: "pipe",
          env: {
            ...process.env,
            HOMEBREW_NO_AUTO_UPDATE: "1",
          },
        });
        output = await new Response(proc.stdout).text();
        await proc.exited;
      } catch {
        return {
          scannerName: "Homebrew Cleanup",
          findings: [],
          totalSize: 0,
          duration: Date.now() - startTime,
        };
      }

      // Parse output — each line that starts with a path is a removable item
      // brew cleanup --dry-run outputs lines like:
      //   Would remove: /opt/homebrew/Cellar/node/20.0.0 (1234 files, 56MB)
      //   or just paths, depending on version
      const lines = output.trim().split("\n").filter(Boolean);
      const findings: Finding[] = [];

      const checks = lines.map(async (line) => {
        // Extract path from the line
        const pathMatch = line.match(/(\/\S+)/);
        if (!pathMatch) return;

        const filePath = pathMatch[1]!;
        if (!(await pathExists(filePath))) return;

        const size = await getSize(filePath);
        const { label, reason } = classifyPath(filePath);

        findings.push({
          path: filePath,
          label,
          size,
          age: 0, // brew cleanup doesn't give age info
          reason,
        });
      });

      await Promise.all(checks);

      // Sort by size descending
      findings.sort((a, b) => b.size - a.size);

      const totalSize = findings.reduce((sum, f) => sum + f.size, 0);

      return {
        scannerName: "Homebrew Cleanup",
        findings,
        totalSize,
        duration: Date.now() - startTime,
      };
    },
  };
}
