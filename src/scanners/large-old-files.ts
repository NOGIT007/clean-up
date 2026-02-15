/**
 * Large/old files scanner.
 * Walks the home directory (limited depth) to find files that are
 * either very large (>100MB) or very old (>365 days since modification).
 */

import type { Finding, Scanner, ScanResult } from "../types";
import { safeReaddirWithTypes } from "../utils/fs";
import { stat, access } from "node:fs/promises";
import { join } from "node:path";

/** Max depth to walk from home directory. */
const MAX_DEPTH = 5;

/** Size threshold: 100 MB. */
const SIZE_THRESHOLD = 100 * 1024 * 1024;

/** Age threshold: 365 days in ms. */
const AGE_THRESHOLD = 365 * 24 * 60 * 60 * 1000;

/** Maximum number of findings to return (avoid overwhelming the user). */
const MAX_FINDINGS = 50;

/**
 * Cache of directories known to be inside a git repo.
 * If a directory contains .git, all its children are "in a project".
 */
const gitRepoCache = new Map<string, boolean>();

/** Check if a path is inside a git repository (has .git ancestor). */
async function isInsideGitRepo(
  dirPath: string,
  home: string,
): Promise<boolean> {
  let current = dirPath;
  while (current.length >= home.length && current !== "/") {
    const cached = gitRepoCache.get(current);
    if (cached !== undefined) return cached;

    try {
      await access(join(current, ".git"));
      // Found .git — mark this and all parents up to home as "in repo"
      gitRepoCache.set(current, true);
      return true;
    } catch {
      // No .git here, keep looking up
    }
    current = current.replace(/\/[^/]+$/, "");
  }
  gitRepoCache.set(dirPath, false);
  return false;
}

/**
 * Directories to skip entirely when walking.
 * These either contain system files, are too deep, or are already
 * handled by other scanners.
 */
const SKIP_DIRS = new Set([
  "Library",
  ".Trash",
  ".git",
  "node_modules",
  ".next",
  ".venv",
  "venv",
  "__pycache__",
  ".cache",
  ".local",
  ".config",
  ".npm",
  ".bun",
  ".nvm",
  ".volta",
  ".rustup",
  ".cargo",
  ".docker",
  ".orbstack",
  "target",
  "dist",
  "build",
  ".gradle",
  "Applications",
  "Photos Library.photoslibrary",
  "Music",
  "Movies",
  ".Spotlight-V100",
  ".fseventsd",
]);

export function createLargeOldFilesScanner(): Scanner {
  return {
    id: "large-old-files",
    name: "Large & Old Files",
    description: "Files >100MB or untouched for >1 year",

    async scan(): Promise<ScanResult> {
      const startTime = Date.now();
      const home = process.env.HOME;
      if (!home) {
        return {
          scannerName: "Large & Old Files",
          findings: [],
          totalSize: 0,
          duration: Date.now() - startTime,
        };
      }

      const findings: Finding[] = [];
      const now = Date.now();

      interface QueueItem {
        path: string;
        depth: number;
      }

      const queue: QueueItem[] = [{ path: home, depth: 0 }];

      while (queue.length > 0 && findings.length < MAX_FINDINGS) {
        const item = queue.shift()!;
        if (item.depth > MAX_DEPTH) continue;

        const entries = await safeReaddirWithTypes(item.path);

        for (const entry of entries) {
          if (findings.length >= MAX_FINDINGS) break;

          // Skip hidden files/dirs and known skip dirs
          if (entry.name.startsWith(".") && entry.isDirectory) {
            if (SKIP_DIRS.has(entry.name)) continue;
            // Skip all hidden dirs at depth 0 (home level)
            if (item.depth === 0) continue;
          }

          if (SKIP_DIRS.has(entry.name)) continue;

          if (entry.isDirectory) {
            queue.push({ path: entry.path, depth: item.depth + 1 });
            continue;
          }

          // It's a file — check size and age
          try {
            const info = await stat(entry.path);
            const age = now - info.mtimeMs;
            const size = info.size;

            const isLarge = size >= SIZE_THRESHOLD;
            const isOld = age >= AGE_THRESHOLD;

            if (!isLarge && !isOld) continue;

            // Files inside git repos must be BOTH large AND old.
            // A big database in an active project is intentional.
            const parentDir = entry.path.replace(/\/[^/]+$/, "");
            const inRepo = await isInsideGitRepo(parentDir, home);
            if (inRepo && !(isLarge && isOld)) continue;

            const reasons: string[] = [];
            if (isLarge)
              reasons.push(
                `Large file (>${Math.round(SIZE_THRESHOLD / 1024 / 1024)}MB)`,
              );
            if (isOld) reasons.push(`Not modified in over a year`);

            const displayPath = entry.path.replace(home, "~");

            findings.push({
              path: entry.path,
              label: displayPath,
              size,
              age,
              reason: reasons.join(" + "),
            });
          } catch {
            // Permission error or broken symlink — skip
          }
        }
      }

      // Sort by size descending
      findings.sort((a, b) => b.size - a.size);

      const totalSize = findings.reduce((sum, f) => sum + f.size, 0);

      return {
        scannerName: "Large & Old Files",
        findings,
        totalSize,
        duration: Date.now() - startTime,
      };
    },
  };
}
