//! `jj` command wrappers. Each function is a single, named jj operation so the
//! orchestration in `main` reads like the steps we described, and so we can
//! swap implementations (e.g. pull-ref fetch instead of fork remotes) later.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::process::{run, run_status};

/// Absolute root of the jj workspace containing `cwd`.
pub fn workspace_root(cwd: &Path) -> Result<PathBuf> {
    let out = run("jj", &["root"], Some(cwd))?;
    Ok(PathBuf::from(out.trim()))
}

/// Whether a remote with `name` already exists.
pub fn remote_exists(root: &Path, name: &str) -> Result<bool> {
    Ok(remotes(root)?.iter().any(|(n, _)| n == name))
}

/// All configured remotes as `(name, url)` pairs, from `jj git remote list`.
pub fn remotes(root: &Path) -> Result<Vec<(String, String)>> {
    let listing = run("jj", &["git", "remote", "list"], Some(root))?;
    Ok(listing
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let url = parts.next()?;
            Some((name.to_string(), url.to_string()))
        })
        .collect())
}

/// Add a git remote.
pub fn remote_add(root: &Path, name: &str, url: &str) -> Result<()> {
    run_status("jj", &["git", "remote", "add", name, url], Some(root))
        .with_context(|| format!("adding remote `{name}` -> {url}"))
}

/// Fetch a remote, optionally restricting to a single branch.
pub fn fetch(root: &Path, remote: &str, branch: Option<&str>) -> Result<()> {
    let mut args = vec!["git", "fetch", "--remote", remote];
    if let Some(b) = branch {
        args.push("--branch");
        args.push(b);
    }
    run_status("jj", &args, Some(root)).with_context(|| format!("fetching from `{remote}`"))
}

/// Add a new workspace named `name` at `dest`, checked out at `revision`.
pub fn workspace_add(root: &Path, name: &str, dest: &Path, revision: &str) -> Result<()> {
    let dest = dest.to_string_lossy();
    run_status(
        "jj",
        &[
            "workspace",
            "add",
            "--name",
            name,
            "--revision",
            revision,
            dest.as_ref(),
        ],
        Some(root),
    )
    .with_context(|| format!("creating workspace `{name}` at {dest}"))
}

/// Forget a workspace by name (used when `--force` recreates one).
pub fn workspace_forget(root: &Path, name: &str) -> Result<()> {
    run_status("jj", &["workspace", "forget", name], Some(root))
        .with_context(|| format!("forgetting workspace `{name}`"))
}

/// Names of existing workspaces.
pub fn workspace_names(root: &Path) -> Result<Vec<String>> {
    let listing = run("jj", &["workspace", "list"], Some(root))?;
    Ok(listing
        .lines()
        .filter_map(|l| l.split(':').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Run the user's `pr-diff` alias inside `workspace`, returning the git-format diff.
pub fn pr_diff(workspace: &Path) -> Result<String> {
    run("jj", &["pr-diff", "--git", "--no-pager"], Some(workspace))
        .context("running `jj pr-diff` in the PR workspace")
}
