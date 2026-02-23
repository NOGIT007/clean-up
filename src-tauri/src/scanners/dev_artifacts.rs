//! Dev artifacts scanner.
//! Finds build artifacts and dependency caches in development projects:
//! node_modules, .next, dist, .venv, target, __pycache__, etc.

use crate::types::{Effort, Finding, ScanResult};
use crate::utils::fs::{get_file_age_sync, get_size_sync};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
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

/// A found artifact path (before size calculation).
struct FoundArtifact {
    path: PathBuf,
    name: String,
    parent_display: String,
    reason: String,
    effort: Effort,
}

/// Phase 1: Fast synchronous BFS walk to find all artifact paths.
/// No size calculation — just locate directories matching artifact names.
fn find_artifacts_sync(home: &str) -> Vec<FoundArtifact> {
    let mut artifacts = Vec::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    let home_path = Path::new(home);

    let entries = match std::fs::read_dir(home_path) {
        Ok(e) => e,
        Err(_) => return artifacts,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        if !is_dir {
            continue;
        }
        if SKIP_DIRS.contains(name.as_str()) {
            continue;
        }
        if name.starts_with('.') && !DEV_ARTIFACT_NAMES.contains_key(name.as_str()) {
            continue;
        }

        if let Some(artifact) = DEV_ARTIFACT_NAMES.get(name.as_str()) {
            artifacts.push(FoundArtifact {
                path: entry.path(),
                name,
                parent_display: "~".to_string(),
                reason: artifact.reason.to_string(),
                effort: artifact.effort.clone(),
            });
            continue;
        }

        queue.push_back((entry.path(), 1));
    }

    while let Some((dir_path, depth)) = queue.pop_front() {
        if depth > MAX_DEPTH {
            continue;
        }

        let entries = match std::fs::read_dir(&dir_path) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            if !is_dir {
                continue;
            }

            if let Some(artifact) = DEV_ARTIFACT_NAMES.get(name.as_str()) {
                let parent_display = dir_path.to_string_lossy().replace(home, "~");
                artifacts.push(FoundArtifact {
                    path: entry.path(),
                    name,
                    parent_display,
                    reason: artifact.reason.to_string(),
                    effort: artifact.effort.clone(),
                });
                continue; // Don't recurse into artifacts
            }

            if SKIP_DIRS.contains(name.as_str()) {
                continue;
            }
            if name.starts_with('.') {
                continue;
            }

            queue.push_back((entry.path(), depth + 1));
        }
    }

    artifacts
}

/// Create and run the dev artifacts scanner.
/// Phase 1: sync walk to find artifact paths (fast).
/// Phase 2: parallel size calculations via spawn_blocking.
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

    // Phase 1: Find all artifact paths (fast sync walk)
    let home_clone = home.clone();
    let artifacts = tokio::task::spawn_blocking(move || find_artifacts_sync(&home_clone))
        .await
        .unwrap_or_default();

    // Phase 2: Calculate sizes in parallel
    let mut handles = Vec::new();
    for artifact in artifacts {
        handles.push(tokio::task::spawn_blocking(move || {
            let size = get_size_sync(&artifact.path);
            if size < MIN_SIZE {
                return None;
            }
            let age = get_file_age_sync(&artifact.path);
            Some(Finding {
                path: artifact.path.to_string_lossy().to_string(),
                label: format!("{} in {}", artifact.name, artifact.parent_display),
                size,
                age,
                reason: artifact.reason,
                effort: Some(artifact.effort),
            })
        }));
    }

    let mut findings = Vec::new();
    for handle in handles {
        if let Ok(Some(f)) = handle.await {
            findings.push(f);
        }
    }

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
