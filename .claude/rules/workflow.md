# Workflow Rules

_Applied when committing, pushing, or releasing._

## Version Bumping

- Every push must include a version bump
- Update version in all three locations:
  1. `package.json` (`version` field)
  2. `src-tauri/Cargo.toml` (`version` field)
  3. `src-tauri/tauri.conf.json` (`version` field)
- Use semver: patch for fixes, minor for features, major for breaking changes

## Local Build & Install

- After any code change, always rebuild and install locally before pushing:
  `bun run build:app && bun run install:app`
- Verify the installed version: `~/.local/bin/clean-up --version`
- User launches from Spotlight — the installed app must always match the source

## Release Notes

- Create a GitHub release (`gh release create`) for every push
- Use conventional commit style in commit messages
- Release notes should summarize user-facing changes

## Git

- Feature branches: `feature/<issue-number>-<description>`
- Never push without user approval
- Merge to main via fast-forward when possible
