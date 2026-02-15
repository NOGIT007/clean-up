/**
 * Dev artifacts scanner.
 * Finds build artifacts and dependency caches in development projects:
 * node_modules, .next, dist, .venv, target, __pycache__, etc.
 */

import type { Finding, Scanner, ScanResult } from "../types";
import { getFileAge, getSize, safeReaddirWithTypes } from "../utils/fs";

/** Directories that are known dev artifacts. */
const DEV_ARTIFACT_NAMES: Record<
  string,
  { reason: string; effort: "none" | "reinstall" }
> = {
  node_modules: {
    reason: "Node.js dependencies — can be reinstalled with npm/bun install",
    effort: "reinstall",
  },
  ".next": {
    reason: "Next.js build cache — regenerated on next build",
    effort: "none",
  },
  ".nuxt": {
    reason: "Nuxt.js build cache — regenerated on next build",
    effort: "none",
  },
  ".svelte-kit": {
    reason: "SvelteKit build cache — regenerated on next build",
    effort: "none",
  },
  ".turbo": {
    reason: "Turborepo cache — regenerated on next build",
    effort: "none",
  },
  dist: {
    reason: "Build output — regenerated on next build",
    effort: "none",
  },
  build: {
    reason: "Build output — regenerated on next build",
    effort: "none",
  },
  out: {
    reason: "Build output — regenerated on next build",
    effort: "none",
  },
  ".venv": {
    reason: "Python virtual environment — recreate with python -m venv",
    effort: "reinstall",
  },
  venv: {
    reason: "Python virtual environment — recreate with python -m venv",
    effort: "reinstall",
  },
  __pycache__: {
    reason: "Python bytecode cache — regenerated automatically",
    effort: "none",
  },
  ".pytest_cache": {
    reason: "Pytest cache — regenerated on next test run",
    effort: "none",
  },
  ".mypy_cache": {
    reason: "Mypy type-checking cache — regenerated automatically",
    effort: "none",
  },
  ".ruff_cache": {
    reason: "Ruff linter cache — regenerated automatically",
    effort: "none",
  },
  target: {
    reason: "Rust/Java build output — regenerated on next build",
    effort: "none",
  },
  ".gradle": {
    reason: "Gradle build cache — regenerated on next build",
    effort: "none",
  },
  ".cargo": {
    reason: "Cargo cache — regenerated on next build",
    effort: "none",
  },
  ".expo": {
    reason: "Expo cache — regenerated on next build",
    effort: "none",
  },
  ".parcel-cache": {
    reason: "Parcel bundler cache — regenerated on next build",
    effort: "none",
  },
  ".webpack": {
    reason: "Webpack cache — regenerated on next build",
    effort: "none",
  },
  ".angular": {
    reason: "Angular build cache — regenerated on next build",
    effort: "none",
  },
  ".cache": {
    reason: "Generic build cache — usually safe to remove",
    effort: "none",
  },
  ".tsbuildinfo": {
    reason: "TypeScript incremental build cache",
    effort: "none",
  },
  coverage: {
    reason: "Test coverage reports — regenerated on next test run",
    effort: "none",
  },
  ".nyc_output": {
    reason: "NYC coverage output — regenerated on next test run",
    effort: "none",
  },
  ".dart_tool": {
    reason: "Dart tool cache — regenerated automatically",
    effort: "none",
  },
  ".pub-cache": {
    reason: "Dart pub cache — regenerated on next pub get",
    effort: "none",
  },
};

/**
 * Directories to skip entirely when walking.
 * These should NOT be treated as project roots.
 */
const SKIP_DIRS = new Set([
  ".Trash",
  ".git",
  "Library",
  ".cache",
  ".local",
  ".config",
  ".npm",
  ".bun",
  ".nvm",
  ".volta",
  ".rustup",
  ".cargo",
]);

/** Maximum depth to walk looking for dev projects. */
const MAX_DEPTH = 5;

/** Minimum size to report (skip tiny artifacts). */
const MIN_SIZE = 1024 * 1024; // 1 MB

/**
 * Walk directories looking for dev artifacts.
 * Uses breadth-first traversal with depth limiting.
 */
async function findDevArtifacts(startDir: string): Promise<Finding[]> {
  const findings: Finding[] = [];

  interface QueueItem {
    path: string;
    depth: number;
  }

  const queue: QueueItem[] = [{ path: startDir, depth: 0 }];

  while (queue.length > 0) {
    const item = queue.shift()!;

    if (item.depth > MAX_DEPTH) continue;

    const entries = await safeReaddirWithTypes(item.path);

    for (const entry of entries) {
      // Skip hidden dirs and known non-project dirs at top level
      if (entry.name.startsWith(".") && !DEV_ARTIFACT_NAMES[entry.name]) {
        if (SKIP_DIRS.has(entry.name)) continue;
      }

      if (!entry.isDirectory) continue;

      const artifact = DEV_ARTIFACT_NAMES[entry.name];
      if (artifact) {
        // Found an artifact — measure it but don't descend into it
        const size = await getSize(entry.path);
        if (size >= MIN_SIZE) {
          const age = await getFileAge(entry.path);
          findings.push({
            path: entry.path,
            label: `${entry.name} in ${item.path.replace(process.env.HOME ?? "", "~")}`,
            size,
            age,
            reason: artifact.reason,
            effort: artifact.effort,
          });
        }
        // Don't recurse into the artifact itself
        continue;
      }

      // Skip known non-project directories
      if (SKIP_DIRS.has(entry.name)) continue;

      // Recurse into this directory
      queue.push({ path: entry.path, depth: item.depth + 1 });
    }
  }

  return findings;
}

export function createDevArtifactsScanner(): Scanner {
  return {
    id: "dev-artifacts",
    name: "Dev Artifacts",
    description: "node_modules, build caches, virtual environments",

    async scan(): Promise<ScanResult> {
      const startTime = Date.now();
      const home = process.env.HOME;
      if (!home) {
        return {
          scannerName: "Dev Artifacts",
          findings: [],
          totalSize: 0,
          duration: Date.now() - startTime,
        };
      }

      const findings = await findDevArtifacts(home);

      // Sort by size descending
      findings.sort((a, b) => b.size - a.size);

      const totalSize = findings.reduce((sum, f) => sum + f.size, 0);

      return {
        scannerName: "Dev Artifacts",
        findings,
        totalSize,
        duration: Date.now() - startTime,
      };
    },
  };
}
