/**
 * Scanner registry.
 * Central place to register and retrieve all available scanners.
 */

import type { Scanner } from "../types";
import { createDevArtifactsScanner } from "./dev-artifacts";
import { createAppLeftoversScanner } from "./app-leftovers";
import { createSystemCachesScanner } from "./system-caches";
import { createLargeOldFilesScanner } from "./large-old-files";
import { createUnusedAppsScanner } from "./unused-apps";
import { createHomebrewCleanupScanner } from "./homebrew-cleanup";

/** Get all available scanners. */
export function getAllScanners(): Scanner[] {
  return [
    createDevArtifactsScanner(),
    createSystemCachesScanner(),
    createAppLeftoversScanner(),
    createLargeOldFilesScanner(),
    createUnusedAppsScanner(),
    createHomebrewCleanupScanner(),
  ];
}
