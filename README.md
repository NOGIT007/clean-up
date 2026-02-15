# Clean Up

Interactive macOS cleanup tool with a native app and web UI. Finds junk files, unused apps, stale caches, and dev artifacts — lets you review everything before moving selected items to Trash.

Zero runtime dependencies. One standalone binary. Everything is recoverable.

## Features

- **6 scanners** — dev artifacts, system caches, app leftovers, large/old files, unused apps, Homebrew cleanup
- **Web UI** — visual interface with grouped results, size/age info, and batch selection
- **App uninstaller** — lists installed apps with icons, finds all associated data (caches, preferences, containers), and removes everything in one click
- **Spotlight maintenance** — check indexing status and trigger reindex from the UI
- **Terminal TUI** — fully interactive terminal mode with arrow-key navigation
- **Dry-run mode** — preview what would be cleaned without touching anything
- **Native .app bundle** — launch from Spotlight like any macOS app

## What It Scans

| Scanner               | What it finds                                                                                         | Details                                              |
| --------------------- | ----------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| **Dev Artifacts**     | `node_modules`, `.next`, `dist`, `.venv`, `target`, build caches                                      | Walks home dir (5 levels deep), min 1 MB             |
| **System Caches**     | Browser caches (Chrome, Firefox, Safari, Arc, etc.), Xcode DerivedData, Homebrew, pip, yarn, npm, Bun | Min 5 MB, flags TCC-protected paths                  |
| **App Leftovers**     | Orphaned preferences, caches, containers from uninstalled apps                                        | Scans ~/Library subdirectories, matches by bundle ID |
| **Large & Old Files** | Files >100 MB or untouched for >1 year                                                                | Smart git detection — won't flag active repo files   |
| **Unused Apps**       | Apps not opened in 6+ months                                                                          | Uses Spotlight metadata, excludes 50+ system apps    |
| **Homebrew**          | Old formula versions, stale downloads                                                                 | Uses `brew cleanup --dry-run`                        |

## Install

Requires [Bun](https://bun.sh) to build (not needed at runtime).

```bash
git clone https://github.com/NOGIT007/clean-up.git
cd clean-up
bun install
bun run build:app
bun run install:app
```

This installs **Clean Up.app** to `~/Applications` (Spotlight-indexed) and symlinks the CLI to `~/.local/bin/clean-up`.

## Usage

### Spotlight (recommended)

Search for **"Clean Up"** in Spotlight. The app launches a local web UI in your browser.

### Terminal

```bash
clean-up            # interactive TUI in terminal
clean-up --web      # launch web UI instead
clean-up --dry-run  # preview without deleting
clean-up --version  # print version
clean-up --help     # show all options
```

## How It Works

1. **Select** — pick which scanners to run (or select all)
2. **Scan** — the tool walks relevant directories and calculates sizes
3. **Review** — browse findings grouped by category, see size and age for each item
4. **Trash** — selected items move to macOS Trash (always recoverable via Finder)

Nothing is permanently deleted. Every action requires your explicit confirmation.

## Web UI

The web interface has three tabs:

- **Clean** — run scanners, review results, select and trash items
- **Uninstall** — browse installed apps with icons, see associated data across ~/Library, remove apps and all their data
- **Spotlight** — check Spotlight indexing status and trigger a reindex (used by the unused apps scanner)

## App Bundle Structure

```
Clean Up.app/Contents/
  Info.plist              Bundle metadata
  PkgInfo                 APPL????
  MacOS/
    clean-up              Native Swift launcher (no Terminal window)
    clean-up-server       Standalone Bun binary (web server + scanners)
  Resources/
    AppIcon.icns          App icon
    ui.html               Web UI (single-file, zero dependencies)
```

The Swift launcher starts the server process with `--web` and opens your browser. No Terminal window appears.

## Security

- **Zero dependencies** — no npm packages at runtime, single compiled binary
- **Never calls `rm`** — everything goes through macOS Trash (recoverable)
- **Path blocklist** — critical system paths (`/System`, `/Library`, `/usr`, etc.) are hardcoded as off-limits
- **Localhost only** — the web server binds to `127.0.0.1` on a random port
- **No network access** — fully offline, no telemetry, no updates
- **You review everything** — nothing is trashed without explicit selection and confirmation

## Uninstall

```bash
rm -rf ~/Applications/Clean\ Up.app
rm -f ~/.local/bin/clean-up
```

No other files are created outside the app bundle.

## First Launch

- **Gatekeeper**: The app isn't notarized. Right-click → **Open** → click **Open** to bypass the warning once.
- **Full Disk Access** (optional): Some scanners (Safari caches, iCloud data) need Full Disk Access. Go to System Settings → Privacy & Security → Full Disk Access and add Clean Up.app.

## Requirements

- macOS 13.0+ (Ventura or later)
- Apple Silicon or Intel Mac (binary matches build architecture)

## License

[MIT](LICENSE)
