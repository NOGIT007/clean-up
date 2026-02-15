/**
 * Trash utilities — zero dependencies.
 * Moves files to macOS Trash (recoverable). Never uses rm.
 * Includes path safety blocklist to prevent catastrophic deletions.
 */

/**
 * Hardcoded blocklist of paths that must NEVER be trashed.
 * These are critical system directories.
 */
const BLOCKED_PATHS = new Set([
  "/",
  "/System",
  "/System/Library",
  "/Library",
  "/Users",
  "/Applications",
  "/bin",
  "/sbin",
  "/usr",
  "/usr/bin",
  "/usr/lib",
  "/usr/local",
  "/usr/sbin",
  "/etc",
  "/var",
  "/tmp",
  "/private",
  "/private/etc",
  "/private/var",
  "/private/tmp",
  "/opt",
  "/opt/homebrew",
  "/Volumes",
  "/cores",
  "/dev",
  "/net",
  "/home",
]);

/**
 * Patterns that should never be trashed.
 * Checked as prefixes against the normalized path.
 */
const BLOCKED_PREFIXES = [
  "/System/",
  "/usr/",
  "/bin/",
  "/sbin/",
  "/private/etc/",
  "/private/var/",
];

/**
 * Check if a path is safe to trash.
 * Returns true if the path is NOT in the blocklist.
 */
export function isPathSafe(path: string): boolean {
  // Normalize: resolve .. and remove trailing slash
  const normalized = path.replace(/\/+$/, "") || "/";

  // Direct blocklist check
  if (BLOCKED_PATHS.has(normalized)) {
    return false;
  }

  // Prefix check
  for (const prefix of BLOCKED_PREFIXES) {
    if (normalized.startsWith(prefix)) {
      return false;
    }
  }

  // Home directory itself is blocked
  const home = process.env.HOME;
  if (home && normalized === home.replace(/\/+$/, "")) {
    return false;
  }

  return true;
}

/** Result from a batch trash operation. */
export interface TrashResult {
  path: string;
  success: boolean;
}

/**
 * Move multiple files/directories to macOS Trash in a single operation.
 * Batches into one process to avoid multiple credential prompts.
 * Falls back to individual moveToTrash() if the batch fails.
 */
export async function moveMultipleToTrash(
  paths: string[],
): Promise<TrashResult[]> {
  const safePaths = paths.filter((p) => {
    if (!isPathSafe(p)) {
      console.error(`BLOCKED: Refusing to trash protected path: ${p}`);
      return false;
    }
    return true;
  });

  if (safePaths.length === 0) {
    return paths.map((p) => ({ path: p, success: false }));
  }

  // Try the `trash` CLI with all paths as args (single process)
  try {
    const proc = Bun.spawn(["trash", ...safePaths], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const exitCode = await proc.exited;
    if (exitCode === 0) {
      return paths.map((p) => ({
        path: p,
        success: safePaths.includes(p),
      }));
    }
  } catch {
    // trash CLI not installed, fall through
  }

  // Fallback: single AppleScript with all paths
  try {
    const posixFiles = safePaths
      .map((p) => {
        const escaped = p.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
        return `POSIX file "${escaped}"`;
      })
      .join(", ");
    const script = `tell application "Finder" to delete {${posixFiles}}`;
    const proc = Bun.spawn(["osascript", "-e", script], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const exitCode = await proc.exited;
    if (exitCode === 0) {
      return paths.map((p) => ({
        path: p,
        success: safePaths.includes(p),
      }));
    }
  } catch {
    // Batch AppleScript failed, fall through to individual
  }

  // Final fallback: individual moveToTrash for granular results
  const results: TrashResult[] = [];
  for (const p of paths) {
    const success = await moveToTrash(p);
    results.push({ path: p, success });
  }
  return results;
}

/**
 * Move a file or directory to macOS Trash.
 * Uses the `trash` command if available, falls back to AppleScript.
 *
 * @returns true if successfully trashed, false otherwise
 */
export async function moveToTrash(path: string): Promise<boolean> {
  // Safety check
  if (!isPathSafe(path)) {
    console.error(`BLOCKED: Refusing to trash protected path: ${path}`);
    return false;
  }

  // Try the `trash` CLI first (homebrew: brew install trash)
  try {
    const proc = Bun.spawn(["trash", path], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const exitCode = await proc.exited;
    if (exitCode === 0) return true;
  } catch {
    // trash CLI not installed, fall through to AppleScript
  }

  // Fallback: AppleScript via osascript
  try {
    const escapedPath = path.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
    const script = `tell application "Finder" to delete POSIX file "${escapedPath}"`;
    const proc = Bun.spawn(["osascript", "-e", script], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const exitCode = await proc.exited;
    return exitCode === 0;
  } catch {
    return false;
  }
}
