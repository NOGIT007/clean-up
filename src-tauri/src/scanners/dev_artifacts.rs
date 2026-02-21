//! Dev artifacts scanner.
//! Finds build artifacts and dependency caches in development projects:
//! node_modules, .next, dist, .venv, target, __pycache__, etc.

use crate::types::{Effort, Finding, ScanResult};
use crate::utils::fs::{get_file_age, get_size, safe_readdir_with_types};
use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::LazyLock;
use std::time::Instant;

/// A known dev artifact directory pattern.
struct ArtifactDef {
    reason: &'static str,
    effort: Effort,
}

/// Directories that are known dev artifacts.
static DEV_ARTIFACT_NAMES: LazyLock<std::collections::HashMap<&'static str, ArtifactDef>> =
    LazyLock::new(|| {
        let mut m = std::collections::HashMap::new();
        m.insert("node_modules", ArtifactDef { reason: "Node.js dependencies \u{2014} can be reinstalled with npm/bun install", effort: Effort::Reinstall });
        m.insert(".next", ArtifactDef { reason: "Next.js build cache \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".nuxt", ArtifactDef { reason: "Nuxt.js build cache \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".svelte-kit", ArtifactDef { reason: "SvelteKit build cache \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".turbo", ArtifactDef { reason: "Turborepo cache \u{2014} regenerated on next build", effort: Effort::None });
        m.insert("dist", ArtifactDef { reason: "Build output \u{2014} regenerated on next build", effort: Effort::None });
        m.insert("build", ArtifactDef { reason: "Build output \u{2014} regenerated on next build", effort: Effort::None });
        m.insert("out", ArtifactDef { reason: "Build output \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".venv", ArtifactDef { reason: "Python virtual environment \u{2014} recreate with python -m venv", effort: Effort::Reinstall });
        m.insert("venv", ArtifactDef { reason: "Python virtual environment \u{2014} recreate with python -m venv", effort: Effort::Reinstall });
        m.insert("__pycache__", ArtifactDef { reason: "Python bytecode cache \u{2014} regenerated automatically", effort: Effort::None });
        m.insert(".pytest_cache", ArtifactDef { reason: "Pytest cache \u{2014} regenerated on next test run", effort: Effort::None });
        m.insert(".mypy_cache", ArtifactDef { reason: "Mypy type-checking cache \u{2014} regenerated automatically", effort: Effort::None });
        m.insert(".ruff_cache", ArtifactDef { reason: "Ruff linter cache \u{2014} regenerated automatically", effort: Effort::None });
        m.insert("target", ArtifactDef { reason: "Rust/Java build output \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".gradle", ArtifactDef { reason: "Gradle build cache \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".cargo", ArtifactDef { reason: "Cargo cache \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".expo", ArtifactDef { reason: "Expo cache \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".parcel-cache", ArtifactDef { reason: "Parcel bundler cache \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".webpack", ArtifactDef { reason: "Webpack cache \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".angular", ArtifactDef { reason: "Angular build cache \u{2014} regenerated on next build", effort: Effort::None });
        m.insert(".cache", ArtifactDef { reason: "Generic build cache \u{2014} usually safe to remove", effort: Effort::None });
        m.insert(".tsbuildinfo", ArtifactDef { reason: "TypeScript incremental build cache", effort: Effort::None });
        m.insert("coverage", ArtifactDef { reason: "Test coverage reports \u{2014} regenerated on next test run", effort: Effort::None });
        m.insert(".nyc_output", ArtifactDef { reason: "NYC coverage output \u{2014} regenerated on next test run", effort: Effort::None });
        m.insert(".dart_tool", ArtifactDef { reason: "Dart tool cache \u{2014} regenerated automatically", effort: Effort::None });
        m.insert(".pub-cache", ArtifactDef { reason: "Dart pub cache \u{2014} regenerated on next pub get", effort: Effort::None });
        m
    });

/// Directories to skip entirely when walking.
static SKIP_DIRS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        ".Trash", ".git", "Library", ".cache", ".local", ".config", ".npm", ".bun", ".nvm",
        ".volta", ".rustup", ".cargo",
    ]
    .into_iter()
    .collect()
});

/// Maximum depth to walk looking for dev projects.
const MAX_DEPTH: usize = 5;

/// Minimum size to report (skip tiny artifacts).
const MIN_SIZE: u64 = 1024 * 1024; // 1 MB

/// Walk directories looking for dev artifacts using BFS with depth limiting.
async fn find_dev_artifacts(start_dir: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();

    let mut queue: VecDeque<(std::path::PathBuf, usize)> = VecDeque::new();
    queue.push_back((start_dir.to_path_buf(), 0));

    while let Some((dir_path, depth)) = queue.pop_front() {
        if depth > MAX_DEPTH {
            continue;
        }

        let entries = safe_readdir_with_types(&dir_path).await;

        for entry in entries {
            // Skip hidden dirs and known non-project dirs at top level
            if entry.name.starts_with('.') && !DEV_ARTIFACT_NAMES.contains_key(entry.name.as_str())
            {
                if SKIP_DIRS.contains(entry.name.as_str()) {
                    continue;
                }
            }

            if !entry.is_directory {
                continue;
            }

            if let Some(artifact) = DEV_ARTIFACT_NAMES.get(entry.name.as_str()) {
                // Found an artifact — measure it but don't descend into it
                let size = get_size(&entry.path).await;
                if size >= MIN_SIZE {
                    let age = get_file_age(&entry.path).await;
                    let parent_display = dir_path
                        .to_string_lossy()
                        .replace(&home, "~");
                    findings.push(Finding {
                        path: entry.path.to_string_lossy().to_string(),
                        label: format!("{} in {}", entry.name, parent_display),
                        size,
                        age,
                        reason: artifact.reason.to_string(),
                        effort: Some(artifact.effort.clone()),
                    });
                }
                continue;
            }

            // Skip known non-project directories
            if SKIP_DIRS.contains(entry.name.as_str()) {
                continue;
            }

            // Recurse into this directory
            queue.push_back((entry.path, depth + 1));
        }
    }

    findings
}

/// Create and run the dev artifacts scanner.
pub async fn scan() -> ScanResult {
    let start = Instant::now();
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            return ScanResult {
                scanner_name: "Dev Artifacts".to_string(),
                findings: Vec::new(),
                total_size: 0,
                duration: 0,
            };
        }
    };

    let mut findings = find_dev_artifacts(Path::new(&home)).await;

    // Sort by size descending
    findings.sort_by(|a, b| b.size.cmp(&a.size));

    let total_size = findings.iter().map(|f| f.size).sum();

    ScanResult {
        scanner_name: "Dev Artifacts".to_string(),
        findings,
        total_size,
        duration: start.elapsed().as_millis() as u64,
    }
}
