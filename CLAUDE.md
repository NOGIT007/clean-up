# Clean Up

Interactive macOS cleanup CLI tool — zero dependencies, standalone binary.

## Architecture

- **Entry point**: `src/index.ts` (TUI flow: select → scan → review → trash)
- **Scanners**: `src/scanners/` (dev-artifacts, system-caches, app-leftovers, large-old-files)
- **TUI**: `src/tui/` (custom prompts + formatting, no deps)
- **Utils**: `src/utils/` (fs helpers, trash via macOS `osascript`, app detection)
- **Build**: `scripts/build.sh` → standalone binary + .app bundle
- **Stack**: TypeScript + Bun (build-time only)

## Critical Rules

- Never use `rm` — all deletions go through `moveToTrash()` in `src/utils/trash.ts`
- Zero runtime dependencies — no npm packages allowed
- Version must be bumped in both `package.json` and `src/index.ts` (VERSION const)
- Path blocklist in `src/utils/fs.ts` must never be weakened

## Detailed Rules

See `.claude/rules/` for context-specific rules:

- `workflow.md` — version bumping, release notes, push protocol
