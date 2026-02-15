/**
 * Unused apps scanner.
 * Detects .app bundles not opened in 6+ months using Spotlight metadata.
 */

import type { Finding, Scanner, ScanResult } from "../types";
import { isSystemBundleId } from "../utils/apps";
import { getSize, safeReaddir } from "../utils/fs";

/** Months of inactivity before flagging an app. */
const STALE_MONTHS = 6;

/** Minimum app size to bother reporting. */
const MIN_SIZE = 1024 * 1024; // 1 MB

/** Batch size for mdls subprocess calls. */
const BATCH_SIZE = 10;

/**
 * Apple apps that don't have standard third-party bundle IDs.
 * These should never be flagged as unused.
 */
const SKIP_APPS = new Set([
  "Safari.app",
  "Mail.app",
  "Terminal.app",
  "Activity Monitor.app",
  "System Preferences.app",
  "System Settings.app",
  "App Store.app",
  "Calculator.app",
  "Calendar.app",
  "Contacts.app",
  "Dictionary.app",
  "Disk Utility.app",
  "FaceTime.app",
  "Finder.app",
  "Font Book.app",
  "Home.app",
  "Keychain Access.app",
  "Maps.app",
  "Messages.app",
  "Migration Assistant.app",
  "Music.app",
  "News.app",
  "Notes.app",
  "Photo Booth.app",
  "Photos.app",
  "Podcasts.app",
  "Preview.app",
  "QuickTime Player.app",
  "Reminders.app",
  "Screenshot.app",
  "Shortcuts.app",
  "Siri.app",
  "Stocks.app",
  "TextEdit.app",
  "Time Machine.app",
  "TV.app",
  "Voice Memos.app",
  "Weather.app",
  "Console.app",
  "Automator.app",
  "Books.app",
  "Chess.app",
  "Clock.app",
  "Freeform.app",
  "Grapher.app",
  "Image Capture.app",
  "Launchpad.app",
  "Mission Control.app",
  "Stickies.app",
  "clean-up.app",
  "Clean Up.app",
]);

/** Get the last-used date of an app via Spotlight metadata. */
async function getLastUsedDate(appPath: string): Promise<Date | null> {
  try {
    const proc = Bun.spawn(["mdls", "-name", "kMDItemLastUsedDate", appPath], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const output = await new Response(proc.stdout).text();
    await proc.exited;

    // Parse: kMDItemLastUsedDate = 2024-01-15 10:30:00 +0000
    const match = output.match(
      /kMDItemLastUsedDate\s*=\s*(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})/,
    );
    if (!match?.[1]) return null;

    return new Date(match[1].replace(" ", "T") + "Z");
  } catch {
    return null;
  }
}

/** Get the bundle ID of an app via Spotlight metadata. */
async function getBundleId(appPath: string): Promise<string | null> {
  try {
    const proc = Bun.spawn(
      ["mdls", "-name", "kMDItemCFBundleIdentifier", appPath],
      { stdout: "pipe", stderr: "pipe" },
    );
    const output = await new Response(proc.stdout).text();
    await proc.exited;

    const match = output.match(/"([^"]+)"/);
    return match?.[1] ?? null;
  } catch {
    return null;
  }
}

/** Format a duration in months (rounded). */
function formatMonths(ms: number): string {
  const months = Math.round(ms / (1000 * 60 * 60 * 24 * 30));
  if (months < 1) return "less than a month";
  if (months === 1) return "1 month";
  return `${months} months`;
}

/** Extract the .app name from a full path. */
function appName(appPath: string): string {
  return appPath.split("/").pop() ?? appPath;
}

export function createUnusedAppsScanner(): Scanner {
  return {
    id: "unused-apps",
    name: "Unused Apps",
    description: "Applications not opened in 6+ months",

    async scan(): Promise<ScanResult> {
      const startTime = Date.now();
      const home = process.env.HOME;
      if (!home) {
        return {
          scannerName: "Unused Apps",
          findings: [],
          totalSize: 0,
          duration: Date.now() - startTime,
        };
      }

      // Gather .app paths from both locations
      const appDirs = ["/Applications", `${home}/Applications`];
      const appPaths: string[] = [];

      for (const dir of appDirs) {
        const entries = await safeReaddir(dir);
        for (const entry of entries) {
          if (entry.endsWith(".app")) {
            appPaths.push(entry);
          }
        }
      }

      const findings: Finding[] = [];
      const cutoff = Date.now() - STALE_MONTHS * 30 * 24 * 60 * 60 * 1000;

      // Process in batches to avoid spawning too many mdls processes
      for (let i = 0; i < appPaths.length; i += BATCH_SIZE) {
        const batch = appPaths.slice(i, i + BATCH_SIZE);
        const promises = batch.map(async (appPath) => {
          const name = appName(appPath);

          // Skip known Apple apps
          if (SKIP_APPS.has(name)) return;

          // Check bundle ID for system apps
          const bundleId = await getBundleId(appPath);
          if (bundleId && isSystemBundleId(bundleId)) return;

          // Skip tiny apps
          const size = await getSize(appPath);
          if (size < MIN_SIZE) return;

          // Check last used date
          const lastUsed = await getLastUsedDate(appPath);
          const now = Date.now();

          if (lastUsed) {
            const lastUsedMs = lastUsed.getTime();
            if (lastUsedMs >= cutoff) return; // Used recently

            const age = now - lastUsedMs;
            findings.push({
              path: appPath,
              label: name.replace(/\.app$/, ""),
              size,
              age,
              reason: `Not opened in ${formatMonths(age)}`,
            });
          } else {
            // No usage data — might be unused
            findings.push({
              path: appPath,
              label: name.replace(/\.app$/, ""),
              size,
              age: now - cutoff, // Approximate
              reason: "No usage data — may be unused",
            });
          }
        });
        await Promise.all(promises);
      }

      // Sort by size descending
      findings.sort((a, b) => b.size - a.size);

      const totalSize = findings.reduce((sum, f) => sum + f.size, 0);

      return {
        scannerName: "Unused Apps",
        findings,
        totalSize,
        duration: Date.now() - startTime,
      };
    },
  };
}
