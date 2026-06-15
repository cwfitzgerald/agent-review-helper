//! Target-repository discovery: where we are, and which GitHub repo `origin`
//! points at. Everything `gh`-facing is keyed off [`RepoRef`] so we never rely
//! on `gh` auto-detecting the repo from a (possibly non-colocated) jj workdir.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::jj;

/// Protocol style of a git remote URL, so we can rebuild fork URLs in kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlStyle {
    Https,
    Ssh,
}

/// A GitHub repository reference, plus enough of the origin URL to construct
/// sibling fork URLs that match the user's protocol/host.
#[derive(Debug, Clone)]
pub struct RepoRef {
    pub host: String,
    pub owner: String,
    pub repo: String,
    pub style: UrlStyle,
}

impl RepoRef {
    /// `owner/repo`, the form `gh --repo` expects.
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    /// Build a clone URL for `fork_owner/fork_repo` on the same host, matching
    /// this repo's protocol style.
    pub fn sibling_url(&self, fork_owner: &str, fork_repo: &str) -> String {
        match self.style {
            UrlStyle::Https => format!("https://{}/{}/{}.git", self.host, fork_owner, fork_repo),
            UrlStyle::Ssh => format!("git@{}:{}/{}.git", self.host, fork_owner, fork_repo),
        }
    }

    /// Parse a git remote URL (https or scp-like ssh) into a [`RepoRef`].
    pub fn parse_url(url: &str) -> Result<Self> {
        let url = url.trim();
        if let Some(rest) = url.strip_prefix("https://") {
            // host/owner/repo(.git)
            let mut parts = rest.splitn(2, '/');
            let host = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default();
            let (owner, repo) = split_owner_repo(path)?;
            return Ok(Self { host, owner, repo, style: UrlStyle::Https });
        }
        if let Some(rest) = url.strip_prefix("git@") {
            // host:owner/repo(.git)
            let mut parts = rest.splitn(2, ':');
            let host = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default();
            let (owner, repo) = split_owner_repo(path)?;
            return Ok(Self { host, owner, repo, style: UrlStyle::Ssh });
        }
        if let Some(rest) = url.strip_prefix("ssh://git@") {
            // host/owner/repo(.git)
            let mut parts = rest.splitn(2, '/');
            let host = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default();
            let (owner, repo) = split_owner_repo(path)?;
            return Ok(Self { host, owner, repo, style: UrlStyle::Ssh });
        }
        bail!("unrecognized remote URL form: {url}");
    }

    /// Override the owner/repo while preserving host + style (for `--repo`).
    pub fn with_slug(mut self, slug: &str) -> Result<Self> {
        let (owner, repo) = split_owner_repo(slug)?;
        self.owner = owner;
        self.repo = repo;
        Ok(self)
    }
}

fn split_owner_repo(path: &str) -> Result<(String, String)> {
    let path = path.trim_end_matches('/');
    let mut parts = path.splitn(2, '/');
    let owner = parts.next().unwrap_or_default().to_string();
    let repo = parts
        .next()
        .unwrap_or_default()
        .trim_end_matches(".git")
        .to_string();
    if owner.is_empty() || repo.is_empty() {
        bail!("could not parse owner/repo from `{path}`");
    }
    Ok((owner, repo))
}

/// The discovered context of the repository we were invoked in.
pub struct TargetRepo {
    /// Absolute path to the jj workspace root we were invoked from.
    pub root: PathBuf,
    /// GitHub reference derived from the `origin` remote.
    pub origin: RepoRef,
}

/// Discover the jj workspace root and the `origin` GitHub reference.
///
/// `slug_override` (from `--repo`) replaces the parsed owner/repo while keeping
/// the origin host/protocol for fork URL construction.
pub fn discover(cwd: &Path, slug_override: Option<&str>) -> Result<TargetRepo> {
    let root = jj::workspace_root(cwd).context(
        "not inside a jj repository (run from your target repo, e.g. the wgpu checkout)",
    )?;

    let origin_url = jj::remote_url(&root, "origin")
        .context("could not read the `origin` remote URL via `jj git remote list`")?;
    let mut origin = RepoRef::parse_url(&origin_url)?;
    if let Some(slug) = slug_override {
        origin = origin.with_slug(slug)?;
    }

    Ok(TargetRepo { root, origin })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https() {
        let r = RepoRef::parse_url("https://github.com/gfx-rs/wgpu.git").unwrap();
        assert_eq!(r.host, "github.com");
        assert_eq!(r.owner, "gfx-rs");
        assert_eq!(r.repo, "wgpu");
        assert_eq!(r.style, UrlStyle::Https);
    }

    #[test]
    fn parses_https_without_suffix() {
        let r = RepoRef::parse_url("https://github.com/gfx-rs/wgpu").unwrap();
        assert_eq!(r.repo, "wgpu");
    }

    #[test]
    fn parses_scp_ssh() {
        let r = RepoRef::parse_url("git@github.com:gfx-rs/wgpu.git").unwrap();
        assert_eq!(r.host, "github.com");
        assert_eq!(r.owner, "gfx-rs");
        assert_eq!(r.repo, "wgpu");
        assert_eq!(r.style, UrlStyle::Ssh);
    }

    #[test]
    fn parses_ssh_url() {
        let r = RepoRef::parse_url("ssh://git@github.com/gfx-rs/wgpu.git").unwrap();
        assert_eq!(r.owner, "gfx-rs");
        assert_eq!(r.style, UrlStyle::Ssh);
    }

    #[test]
    fn sibling_url_matches_style() {
        let https = RepoRef::parse_url("https://github.com/gfx-rs/wgpu.git").unwrap();
        assert_eq!(
            https.sibling_url("contributor", "wgpu"),
            "https://github.com/contributor/wgpu.git"
        );
        let ssh = RepoRef::parse_url("git@github.com:gfx-rs/wgpu.git").unwrap();
        assert_eq!(
            ssh.sibling_url("contributor", "wgpu"),
            "git@github.com:contributor/wgpu.git"
        );
    }

    #[test]
    fn with_slug_preserves_host_and_style() {
        let r = RepoRef::parse_url("git@example.com:a/b.git")
            .unwrap()
            .with_slug("c/d")
            .unwrap();
        assert_eq!(r.host, "example.com");
        assert_eq!(r.owner, "c");
        assert_eq!(r.repo, "d");
        assert_eq!(r.style, UrlStyle::Ssh);
    }

    #[test]
    fn rejects_garbage() {
        assert!(RepoRef::parse_url("not-a-url").is_err());
        assert!(RepoRef::parse_url("https://github.com/onlyowner").is_err());
    }
}
