# Installing Clean Up on a New Mac

## Prerequisites

### 1. Install Xcode Command Line Tools

```bash
xcode-select --install
```

### 2. Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Verify:

```bash
rustc --version   # 1.75+ required
cargo --version
```

### 3. Install Tauri CLI

```bash
cargo install tauri-cli
```

### 4. Install Bun (build script helper)

```bash
curl -fsSL https://bun.sh/install | bash
```

## Build and Install

### Clone the repo

```bash
git clone https://github.com/NOGIT007/clean-up.git
cd clean-up
```

### Build the .app bundle

```bash
bun run build:app
```

This compiles the Rust backend, bundles the frontend, generates app icons, code-signs the binary, and produces `dist/Clean Up.app`.

First build takes 2-5 minutes (downloading and compiling Rust dependencies). Subsequent builds are much faster.

### Install to ~/Applications

```bash
bun run install:app
```

This copies the app to `~/Applications/`, removes the dist copy to prevent duplicate Spotlight entries, and creates a CLI symlink at `~/.local/bin/clean-up`.

## macOS Permissions

Clean Up needs **Full Disk Access** to scan protected directories like `~/Library`.

### Grant Full Disk Access

1. Open **System Settings**
2. Go to **Privacy & Security > Full Disk Access**
3. Click the **+** button
4. Navigate to `~/Applications/Clean Up.app` and add it
5. Toggle it **on**

> Full Disk Access resets after every rebuild because macOS ties the permission to the specific binary signature. You must re-grant it after each `bun run install:app`.

### Finder Automation (automatic)

The app uses `osascript` to move files to Trash via Finder. macOS will prompt you to allow this on first use. Click **OK** to grant it.

## Launching

### Spotlight

Press **Cmd + Space**, type **Clean Up**, and hit Enter. Spotlight indexes `~/Applications/` automatically. If the app doesn't appear immediately, wait a few seconds for indexing or run:

```bash
mdimport ~/Applications/Clean\ Up.app
```

### CLI

If `~/.local/bin` is in your PATH:

```bash
clean-up
```

This launches the GUI app.

### Direct

```bash
open ~/Applications/Clean\ Up.app
```

## First Launch

On first launch, macOS Gatekeeper may block the app because it's ad-hoc signed (not notarized). To bypass:

1. Right-click `Clean Up.app` in Finder
2. Select **Open**
3. Click **Open** in the dialog

This only needs to be done once.

## Development

### Run in dev mode (hot reload)

```bash
bun run dev
```

### Run tests

```bash
bun run test
```

### Project structure

```
clean-up/
  src-tauri/          Rust backend (scanners, utils, IPC commands)
    src/
      lib.rs          Tauri app setup, appicon:// protocol
      commands.rs     11 IPC commands
      types.rs        Shared types
      scanners/       6 scanner modules
      utils/          fs, trash, apps helpers
    Cargo.toml
    tauri.conf.json   Bundle config, CSP, window settings
    Entitlements.plist
    icons/            App icons (all sizes)
  frontend/
    index.html        Single-page app (embedded in Tauri webview)
  scripts/
    build.sh          Build pipeline
    install.sh        Install to ~/Applications
    generate-icon.swift   Programmatic icon generation
```

## Troubleshooting

**App doesn't appear in Spotlight:**
Run `mdimport ~/Applications/Clean\ Up.app` and wait 10 seconds.

**"Clean Up" is damaged and can't be opened:**
Right-click > Open, or run: `xattr -cr ~/Applications/Clean\ Up.app`

**Scanners show 0 results:**
Grant Full Disk Access (see permissions section above).

**Build fails with "tauri not found":**
Run `cargo install tauri-cli` and ensure `~/.cargo/bin` is in your PATH.

**Build fails with linker errors:**
Ensure Xcode Command Line Tools are installed: `xcode-select --install`
