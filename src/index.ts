#!/usr/bin/env bun

/**
 * clean-up -- Interactive macOS cleanup CLI tool
 *
 * Zero dependencies. Everything goes to Trash (recoverable).
 * The user reviews and approves every deletion.
 */

import type { CliOptions, Finding, Scanner, ScanResult } from "./types";
import {
  intro,
  outro,
  note,
  warn,
  multiselect,
  confirm,
  spinner,
  summary,
} from "./tui/prompts";
import { colors, formatBytes, formatAge, truncatePath } from "./tui/format";
import { moveMultipleToTrash } from "./utils/trash";
import { getAllScanners } from "./scanners/registry";
import { VERSION } from "./version";

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

/** Detect if running inside a .app bundle (macOS). */
function isAppBundle(): boolean {
  return process.execPath.includes(".app/Contents/");
}

function parseArgs(argv: string[]): CliOptions {
  const args = argv.slice(2); // skip bun and script path
  return {
    help: args.includes("--help") || args.includes("-h"),
    version: args.includes("--version") || args.includes("-v"),
    dryRun: args.includes("--dry-run") || args.includes("-n"),
    web: args.includes("--web") || args.includes("-w") || isAppBundle(),
  };
}

function printHelp(): void {
  console.log(`
clean-up v${VERSION}
Interactive macOS cleanup tool -- zero dependencies

Usage:
  clean-up [options]

Options:
  -h, --help      Show this help message
  -v, --version   Show version number
  -n, --dry-run   Preview what would be cleaned without trashing
  -w, --web       Launch browser UI instead of terminal TUI

Categories scanned:
  - Dev Artifacts    node_modules, .next, dist, .venv, target, etc.
  - System Caches    Browser caches, Xcode, Homebrew, pip, yarn, npm
  - App Leftovers    Orphaned data from uninstalled applications
  - Large & Old      Files >100MB or untouched for >1 year
  - Unused Apps      Applications not opened in 6+ months
  - Homebrew         Old formula versions and stale cache

Everything goes to Trash (recoverable). You review and approve every action.
`);
}

// ---------------------------------------------------------------------------
// Terminal safety: restore terminal state on exit
// ---------------------------------------------------------------------------

function setupTerminalSafety(): void {
  const restore = () => {
    process.stdout.write("\x1b[?25h"); // show cursor
    if (process.stdin.isRaw) {
      try {
        process.stdin.setRawMode(false);
      } catch {
        // ignore
      }
    }
  };

  process.on("exit", restore);
  process.on("SIGINT", () => {
    restore();
    console.log();
    process.exit(0);
  });
  process.on("SIGTERM", () => {
    restore();
    process.exit(0);
  });
}

// ---------------------------------------------------------------------------
// Main flow
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
  const options = parseArgs(process.argv);

  // Handle --help and --version before setting up terminal
  if (options.help) {
    printHelp();
    return;
  }

  if (options.version) {
    console.log(`clean-up v${VERSION}`);
    return;
  }

  // Web UI mode — launch HTTP server + browser
  if (options.web) {
    const { startWebServer } = await import("./web/server");
    await startWebServer(options);
    return;
  }

  setupTerminalSafety();

  intro(`clean-up v${VERSION}`);

  if (options.dryRun) {
    note("DRY RUN MODE", "No files will be moved to Trash.");
  }

  note(
    "Interactive macOS cleanup tool",
    "Everything goes to Trash (recoverable).\nYou review and approve every deletion.",
  );

  // Step 1: Let user choose which scanners to run
  const scanners = getAllScanners();

  const selectedScanners = await multiselect<Scanner>(
    "Which categories do you want to scan?",
    scanners.map((s) => ({
      label: s.name,
      value: s,
      hint: s.description,
      selected: true,
    })),
  );

  if (selectedScanners.length === 0) {
    outro("Nothing selected. Bye!");
    return;
  }

  // Step 2: Run selected scanners
  const allResults: ScanResult[] = [];

  for (const scanner of selectedScanners) {
    const s = spinner(`Scanning: ${scanner.name}`);

    try {
      const result = await scanner.scan();
      allResults.push(result);
      s.stop(
        `${scanner.name}: found ${result.findings.length} items (${formatBytes(result.totalSize)})`,
      );
    } catch (err) {
      s.stop(`${scanner.name}: error during scan`);
      warn(`Scanner "${scanner.name}" failed: ${err}`);
    }
  }

  // Combine all findings
  const allFindings = allResults.flatMap((r) => r.findings);

  if (allFindings.length === 0) {
    outro("Nothing found to clean up. Your system is already clean!");
    return;
  }

  // Step 3: Show summary of findings
  const totalSize = allFindings.reduce((sum, f) => sum + f.size, 0);

  summary(
    `Found ${allFindings.length} items (${formatBytes(totalSize)} total)`,
    allResults
      .filter((r) => r.findings.length > 0)
      .map((r) => ({
        label: `${r.scannerName} (${r.findings.length} items)`,
        size: r.totalSize,
      })),
  );

  // Step 4: Let user review and select items to delete
  const itemsToReview = allFindings.map((f) => ({
    label: `${truncatePath(f.path, 50)}  ${colors.yellow(formatBytes(f.size))}  ${colors.dim((f.effort === "reinstall" ? "[reinstall] " : "") + f.reason)}`,
    value: f,
    hint: formatAge(f.age) + " old",
    selected: true,
  }));

  const selectedItems = await multiselect<Finding>(
    "Select items to move to Trash:",
    itemsToReview,
  );

  if (selectedItems.length === 0) {
    outro("Nothing selected for cleanup. Bye!");
    return;
  }

  // Step 5: Final confirmation
  const selectedSize = selectedItems.reduce((sum, f) => sum + f.size, 0);

  if (options.dryRun) {
    // Dry run: just show what would be done
    note(
      "DRY RUN -- would move to Trash:",
      selectedItems
        .map((f) => `${truncatePath(f.path, 60)}  (${formatBytes(f.size)})`)
        .join("\n"),
    );
    outro(
      `Dry run complete. Would free ${formatBytes(selectedSize)} by moving ${selectedItems.length} items to Trash.`,
    );
    return;
  }

  const confirmed = await confirm(
    `Move ${selectedItems.length} items (${formatBytes(selectedSize)}) to Trash?`,
  );

  if (!confirmed) {
    outro("Cancelled. Nothing was moved to Trash.");
    return;
  }

  // Step 6: Move to trash (batch to avoid multiple credential prompts)
  const s = spinner(`Moving ${selectedItems.length} items to Trash`);

  const trashResults = await moveMultipleToTrash(
    selectedItems.map((item) => item.path),
  );

  let trashed = 0;
  let failed = 0;
  let freedSize = 0;

  for (let i = 0; i < trashResults.length; i++) {
    const result = trashResults[i]!;
    if (result.success) {
      trashed++;
      freedSize += selectedItems[i]!.size;
    } else {
      failed++;
      warn(`Failed to trash: ${result.path}`);
    }
  }

  s.stop(`Done! Moved ${trashed} items to Trash`);

  // Step 7: Final summary
  if (failed > 0) {
    warn(`${failed} items could not be moved to Trash`);
  }

  outro(
    `Freed ${formatBytes(freedSize)} by moving ${trashed} items to Trash. All items are recoverable from Trash.`,
  );
}

main().catch((err) => {
  // Restore terminal
  process.stdout.write("\x1b[?25h");
  console.error("Fatal error:", err);
  process.exit(1);
});
