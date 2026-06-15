//! Filesystem-side setup: ensuring git/jj ignore the artifact dir, and
//! preparing (or force-recreating) the per-PR folders and jj workspace.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::jj;
use crate::layout::{Layout, Storage};

/// Deterministic jj workspace name for a bundle, so `--force`/`--clean` can
/// forget it. `bundle` is the per-change folder id (e.g. `pr-9464`,
/// `range-<from>-<to>`), giving names like `arh-pr-9464`.
pub fn workspace_name(bundle: &str) -> String {
    format!("arh-{bundle}")
}

/// Ensure `.agent-review/` is ignored by both git and jj, so the parent
/// workspace never tries to snapshot a nested checkout.
///
/// Writes to the git exclude file (works for git and, when colocated, for jj).
/// Idempotent.
pub fn ensure_ignored(repo_root: &Path) -> Result<()> {
    let Some(exclude) = git_exclude_path(repo_root) else {
        // No git backend found; nothing we can safely write to.
        return Ok(());
    };
    let entry = ".agent-review/";
    let existing = fs::read_to_string(&exclude).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == entry) {
        return Ok(());
    }
    if let Some(parent) = exclude.parent() {
        fs::create_dir_all(parent).ok();
    }
    let mut contents = existing;
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str("# agent-review-helper artifacts\n");
    contents.push_str(entry);
    contents.push('\n');
    fs::write(&exclude, contents)
        .with_context(|| format!("writing ignore entry to {}", exclude.display()))
}

/// Locate the git exclude file: colocated `<repo>/.git/info/exclude` if present,
/// otherwise jj's backing git store exclude.
fn git_exclude_path(repo_root: &Path) -> Option<PathBuf> {
    let colocated = repo_root.join(".git");
    if colocated.is_dir() {
        return Some(colocated.join("info").join("exclude"));
    }
    let backing = repo_root.join(".jj").join("repo").join("store").join("git");
    if backing.is_dir() {
        return Some(backing.join("info").join("exclude"));
    }
    None
}

/// Prepare the artifact folders, handling `--force` by removing prior state
/// (including forgetting the jj workspace). Returns after `info/` exists.
pub fn prepare(layout: &Layout, repo_root: &Path, ws_name: &str, force: bool) -> Result<()> {
    if layout.exists() {
        if !force {
            anyhow::bail!(
                "artifacts already exist (e.g. {}). Re-run with --force to recreate.",
                layout.roots[0].display()
            );
        }
        teardown(layout, repo_root, ws_name)?;
    }
    fs::create_dir_all(&layout.info)
        .with_context(|| format!("creating info dir {}", layout.info.display()))?;
    if let Some(parent) = layout.workspace.parent() {
        fs::create_dir_all(parent).ok();
    }
    Ok(())
}

/// Remove this bundle's folders + jj workspace if they exist. Returns whether
/// anything was actually removed (so the CLI can report a no-op).
pub fn clean(layout: &Layout, repo_root: &Path, ws_name: &str) -> Result<bool> {
    let had_workspace = jj::workspace_names(repo_root)
        .map(|names| names.iter().any(|n| n == ws_name))
        .unwrap_or(false);
    let had_files = layout.exists() || layout.workspace.exists();
    if !had_workspace && !had_files {
        return Ok(false);
    }
    teardown(layout, repo_root, ws_name)?;
    Ok(true)
}

/// Remove a prior bundle folder + jj workspace.
fn teardown(layout: &Layout, repo_root: &Path, ws_name: &str) -> Result<()> {
    if jj::workspace_names(repo_root)
        .map(|names| names.iter().any(|n| n == ws_name))
        .unwrap_or(false)
    {
        jj::workspace_forget(repo_root, ws_name)?;
    }
    for root in &layout.roots {
        if root.exists() {
            fs::remove_dir_all(root)
                .with_context(|| format!("removing {}", root.display()))?;
        }
    }
    // Split keeps the workspace outside `roots`; clean it up explicitly.
    if layout.strategy == Storage::Split && layout.workspace.exists() {
        fs::remove_dir_all(&layout.workspace).ok();
    }
    Ok(())
}
