/**
 * Filesystem utilities — zero dependencies.
 * Provides safe directory reading, size calculation, and file age.
 */

import { readdir, stat } from "node:fs/promises";
import { join } from "node:path";

/**
 * Get the size of a file or directory in bytes.
 * For directories, shells out to `du -sk` for accuracy.
 */
export async function getSize(path: string): Promise<number> {
  try {
    const info = await stat(path);

    if (info.isFile()) {
      return info.size;
    }

    // For directories, use du -sk (size in KB)
    const proc = Bun.spawn(["du", "-sk", path], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const output = await new Response(proc.stdout).text();
    await proc.exited;

    const kb = parseInt(output.trim().split("\t")[0] || "0", 10);
    return Number.isNaN(kb) ? 0 : kb * 1024;
  } catch {
    return 0;
  }
}

/**
 * Get the age of a file/directory in milliseconds since last modification.
 */
export async function getFileAge(path: string): Promise<number> {
  try {
    const info = await stat(path);
    return Date.now() - info.mtimeMs;
  } catch {
    return 0;
  }
}

/**
 * Safely read a directory's contents.
 * Returns empty array on permission errors or if path doesn't exist.
 */
export async function safeReaddir(path: string): Promise<string[]> {
  try {
    const entries = await readdir(path);
    return entries.map((e) => join(path, e));
  } catch {
    return [];
  }
}

/**
 * Safely read directory entries with types.
 * Returns empty array on error.
 */
export async function safeReaddirWithTypes(
  path: string,
): Promise<{ name: string; path: string; isDirectory: boolean }[]> {
  try {
    const entries = await readdir(path, { withFileTypes: true });
    return entries.map((e) => ({
      name: e.name,
      path: join(path, e.name),
      isDirectory: e.isDirectory(),
    }));
  } catch {
    return [];
  }
}

/**
 * Check if a path exists.
 */
export async function pathExists(path: string): Promise<boolean> {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

/**
 * Check if a path is a directory.
 */
export async function isDirectory(path: string): Promise<boolean> {
  try {
    const info = await stat(path);
    return info.isDirectory();
  } catch {
    return false;
  }
}
