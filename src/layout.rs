//! Where artifacts and the PR workspace live on disk.
//!
//! This is deliberately the *only* place that decides paths. The gather logic
//! takes a resolved [`Layout`] and never reasons about storage, so switching
//! strategy (in-repo vs central cache vs split) is a change confined here.

use anyhow::{Context, Result};
use clap::ValueEnum;
use std::path::{Path, PathBuf};

/// Storage strategy. The CLI default is [`Storage::InRepo`]; the others exist so
/// we can move the (large) workspace out of the repo tree without touching any
/// gather code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Storage {
    /// Everything under `<repo>/.agent-review/pr-<n>/`.
    InRepo,
    /// Everything under `<cache>/agent-review-helper/<owner>-<repo>/pr-<n>/`.
    Cache,
    /// Workspace in the cache, info folder in the repo.
    Split,
}

/// Resolved, absolute paths for one PR's artifacts.
#[derive(Debug, Clone)]
pub struct Layout {
    pub strategy: Storage,
    /// The PR workspace checkout.
    pub workspace: PathBuf,
    /// The folder of info artifacts (diffs, conversation, README).
    pub info: PathBuf,
    /// Marker root we treat as "this PR's folder" for existence/--force checks.
    /// For `Split` there are two, so we track both.
    pub roots: Vec<PathBuf>,
}

impl Layout {
    /// Resolve paths for one bundle. `namespace` is the per-repo cache subdir
    /// (e.g. `owner-repo`), `bundle` is the per-change folder (e.g. `pr-9464` or
    /// `range-<from>-<to>`).
    pub fn resolve(strategy: Storage, repo_root: &Path, namespace: &str, bundle: &str) -> Result<Self> {
        let in_repo_root = repo_root.join(".agent-review").join(bundle);
        let cache_root = cache_dir()?
            .join("agent-review-helper")
            .join(namespace)
            .join(bundle);

        let layout = match strategy {
            Storage::InRepo => Layout {
                strategy,
                workspace: in_repo_root.join("workspace"),
                info: in_repo_root.join("info"),
                roots: vec![in_repo_root],
            },
            Storage::Cache => Layout {
                strategy,
                workspace: cache_root.join("workspace"),
                info: cache_root.join("info"),
                roots: vec![cache_root],
            },
            Storage::Split => Layout {
                strategy,
                workspace: cache_root.join("workspace"),
                info: in_repo_root.join("info"),
                roots: vec![in_repo_root, cache_root],
            },
        };
        Ok(layout)
    }

    /// True if any artifact root already exists on disk.
    pub fn exists(&self) -> bool {
        self.roots.iter().any(|r| r.exists())
    }
}

/// Per-user cache directory (`%LOCALAPPDATA%` on Windows, `$XDG_CACHE_HOME`/`~/.cache`
/// elsewhere). Avoids a `dirs` dependency for this single use.
fn cache_dir() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(local));
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        return Ok(PathBuf::from(xdg));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .context("could not determine a home directory for the cache location")?;
    Ok(PathBuf::from(home).join(".cache"))
}
