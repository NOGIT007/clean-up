# Workflow Rules

_Applied when committing, pushing, or releasing._

## Version Bumping

- Every push must include a version bump
- Update version in **both** `package.json` and `src/index.ts` (VERSION const)
- Also update `scripts/build.sh` (Info.plist version strings)
- Use semver: patch for fixes, minor for features, major for breaking changes

## Release Notes

- Create release notes for every push
- Use conventional commit style in commit messages
- Release notes should summarize user-facing changes

## Git

- Feature branches: `feature/<issue-number>-<description>`
- Never push without user approval
- Merge to main via fast-forward when possible
