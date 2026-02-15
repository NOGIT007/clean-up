/**
 * macOS app detection utilities.
 * Uses mdls to find installed app bundle IDs without dependencies.
 */

import type { AppInfo, AppUninstallData } from "../types";
import { getSize, safeReaddir } from "./fs";
import { basename } from "node:path";

/**
 * Get a set of currently installed app bundle identifiers.
 * Uses `mdls` to query Spotlight for applications.
 */
export async function getInstalledApps(): Promise<Set<string>> {
  const bundleIds = new Set<string>();

  try {
    // Find all .app bundles via mdfind
    const proc = Bun.spawn(
      ["mdfind", "kMDItemContentTypeTree == com.apple.application-bundle"],
      { stdout: "pipe", stderr: "pipe" },
    );
    const output = await new Response(proc.stdout).text();
    await proc.exited;

    const appPaths = output.trim().split("\n").filter(Boolean);

    // Get bundle IDs in batches using mdls
    const batchSize = 50;
    for (let i = 0; i < appPaths.length; i += batchSize) {
      const batch = appPaths.slice(i, i + batchSize);
      const promises = batch.map(async (appPath) => {
        try {
          const mdls = Bun.spawn(
            ["mdls", "-name", "kMDItemCFBundleIdentifier", appPath],
            { stdout: "pipe", stderr: "pipe" },
          );
          const mdlsOutput = await new Response(mdls.stdout).text();
          await mdls.exited;

          // Parse: kMDItemCFBundleIdentifier = "com.example.App"
          const match = mdlsOutput.match(/"([^"]+)"/);
          if (match?.[1]) {
            bundleIds.add(match[1].toLowerCase());
          }
        } catch {
          // Skip apps we can't read
        }
      });
      await Promise.all(promises);
    }
  } catch {
    // mdfind not available, return empty set
  }

  return bundleIds;
}

/**
 * Extract a likely bundle ID from a directory name.
 * E.g. "com.spotify.client" from a directory named "com.spotify.client"
 */
export function extractBundleId(dirName: string): string | null {
  // Match reverse-DNS style: com.example.app, org.mozilla.firefox
  if (/^[a-zA-Z][a-zA-Z0-9-]*\.[a-zA-Z][a-zA-Z0-9.-]*$/.test(dirName)) {
    return dirName.toLowerCase();
  }
  return null;
}

/**
 * Check if a bundle ID belongs to a system/Apple service.
 * These should never be flagged as orphans.
 */
export function isSystemBundleId(bundleId: string): boolean {
  const systemPrefixes = [
    "com.apple.",
    "com.microsoft.rdc", // Remote Desktop is a system component
    "group.com.apple.",
    "systemgroup.",
  ];

  const lower = bundleId.toLowerCase();
  return systemPrefixes.some((prefix) => lower.startsWith(prefix));
}

/** Minimum app size to show in the uninstaller list. */
const MIN_APP_SIZE = 1024 * 1024; // 1 MB

/**
 * Get a list of installed non-system apps with display name, path,
 * bundle ID, and size. Filters out system apps and tiny bundles.
 */
export async function getInstalledAppsList(): Promise<AppInfo[]> {
  const proc = Bun.spawn(
    ["mdfind", "kMDItemContentTypeTree == com.apple.application-bundle"],
    { stdout: "pipe", stderr: "pipe" },
  );
  const output = await new Response(proc.stdout).text();
  await proc.exited;

  const appPaths = output
    .trim()
    .split("\n")
    .filter(
      (p) =>
        p &&
        !p.startsWith("/System/") &&
        !p.includes("/Library/Apple/") &&
        p.endsWith(".app"),
    );

  const apps: AppInfo[] = [];
  const batchSize = 50;

  for (let i = 0; i < appPaths.length; i += batchSize) {
    const batch = appPaths.slice(i, i + batchSize);
    const results = await Promise.all(
      batch.map(async (appPath) => {
        try {
          const mdls = Bun.spawn(
            [
              "mdls",
              "-name",
              "kMDItemCFBundleIdentifier",
              "-name",
              "kMDItemDisplayName",
              appPath,
            ],
            { stdout: "pipe", stderr: "pipe" },
          );
          const mdlsOutput = await new Response(mdls.stdout).text();
          await mdls.exited;

          const idMatch = mdlsOutput.match(
            /kMDItemCFBundleIdentifier\s*=\s*"([^"]+)"/,
          );
          const nameMatch = mdlsOutput.match(
            /kMDItemDisplayName\s*=\s*"([^"]+)"/,
          );

          const bundleId = idMatch?.[1];
          if (!bundleId) return null;
          if (isSystemBundleId(bundleId)) return null;

          const displayName =
            nameMatch?.[1] || basename(appPath).replace(/\.app$/, "");
          const appSize = await getSize(appPath);
          if (appSize < MIN_APP_SIZE) return null;

          return { name: displayName, path: appPath, bundleId, appSize };
        } catch {
          return null;
        }
      }),
    );
    for (const r of results) if (r) apps.push(r);
  }

  apps.sort((a, b) => a.name.localeCompare(b.name));
  return apps;
}

/** ~/Library subdirectories to scan for associated app data. */
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

/**
 * Find all associated data in ~/Library for a given app.
 * Matches by bundle ID, app name, and plist files.
 */
export async function getAppAssociatedData(
  bundleId: string,
  appName: string,
): Promise<AppUninstallData[]> {
  const home = process.env.HOME;
  if (!home) return [];

  const results: AppUninstallData[] = [];
  const idLower = bundleId.toLowerCase();
  const nameLower = appName.toLowerCase().replace(/\.app$/, "");

  for (const subdir of LIBRARY_SUBDIRS) {
    const dirPath = `${home}/Library/${subdir}`;
    const entries = await safeReaddir(dirPath);

    for (const entryPath of entries) {
      const name = basename(entryPath);
      const lower = name.toLowerCase();

      // Match by bundle ID (exact, case-insensitive)
      const isBundleMatch = lower === idLower;
      // Match by app name
      const isNameMatch = lower === nameLower || lower === `${nameLower}.app`;
      // Match plist by bundle ID in Preferences
      const isPlistMatch =
        subdir === "Preferences" &&
        (lower === `${idLower}.plist` || lower === `${idLower}.savedstate`);

      if (!isBundleMatch && !isNameMatch && !isPlistMatch) continue;

      const size = await getSize(entryPath);
      results.push({
        path: entryPath,
        label: `${subdir}/${name}`,
        size,
      });
    }
  }

  results.sort((a, b) => b.size - a.size);
  return results;
}
