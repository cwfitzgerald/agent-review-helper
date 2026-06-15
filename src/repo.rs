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
    /// Name of the base remote (the PR's upstream), e.g. `origin` or `upstream`.
    pub remote: String,
    /// GitHub reference derived from the base remote.
    pub origin: RepoRef,
}

/// Discover the jj workspace root and the base GitHub reference.
///
/// The base remote is chosen by [`pick_base_remote`]: `remote_override` if given,
/// else the first of `origin`/`upstream` present, else the sole remote.
/// `slug_override` (from `--repo`) replaces the parsed owner/repo while keeping
/// the base host/protocol for fork URL construction.
pub fn discover(
    cwd: &Path,
    slug_override: Option<&str>,
    remote_override: Option<&str>,
) -> Result<TargetRepo> {
    let root = jj::workspace_root(cwd).context(
        "not inside a jj repository (run from your target repo, e.g. the wgpu checkout)",
    )?;

    let remotes = jj::remotes(&root).context("listing remotes via `jj git remote list`")?;
    let (remote, url) = pick_base_remote(&remotes, remote_override)?;

    let mut origin = RepoRef::parse_url(&url)?;
    if let Some(slug) = slug_override {
        origin = origin.with_slug(slug)?;
    }

    Ok(TargetRepo { root, remote, origin })
}

/// Choose which remote is the PR's base ("upstream") repo, returning its
/// `(name, url)`. Preference: explicit override, then `origin`, then `upstream`,
/// then — if there is exactly one remote — that one. Otherwise it's ambiguous
/// and the caller must pass `--remote`.
fn pick_base_remote(
    remotes: &[(String, String)],
    remote_override: Option<&str>,
) -> Result<(String, String)> {
    let find = |name: &str| {
        remotes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(n, u)| (n.clone(), u.clone()))
    };

    if let Some(name) = remote_override {
        return find(name).with_context(|| {
            format!("remote `{name}` not found; available: {}", remote_names(remotes))
        });
    }
    if let Some(found) = find("origin").or_else(|| find("upstream")) {
        return Ok(found);
    }
    if let [(name, url)] = remotes {
        return Ok((name.clone(), url.clone()));
    }
    if remotes.is_empty() {
        bail!("no git remotes found; the repo needs an `origin`/`upstream` to identify the PR's source");
    }
    bail!(
        "no `origin` or `upstream` remote, and multiple remotes exist ({}). \
         Pass --remote <name> to pick the PR's base repo.",
        remote_names(remotes)
    )
}

fn remote_names(remotes: &[(String, String)]) -> String {
    remotes
        .iter()
        .map(|(n, _)| n.as_str())
        .collect::<Vec<_>>()
        .join(", ")
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

    fn remotes(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(n, u)| (n.to_string(), u.to_string())).collect()
    }

    #[test]
    fn base_remote_prefers_origin() {
        let r = remotes(&[("upstream", "git@github.com:gfx-rs/wgpu.git"), ("origin", "u2")]);
        assert_eq!(pick_base_remote(&r, None).unwrap().0, "origin");
    }

    #[test]
    fn base_remote_falls_back_to_upstream() {
        // wgpu-style: no `origin`, fork remotes named after owners.
        let r = remotes(&[
            ("teoxoy", "git@ssh.github.com:teoxoy/wgpu"),
            ("upstream", "git@github.com:gfx-rs/wgpu.git"),
            ("cwfitzgerald", "git@github.com:cwfitzgerald/wgpu.git"),
        ]);
        let (name, url) = pick_base_remote(&r, None).unwrap();
        assert_eq!(name, "upstream");
        assert_eq!(url, "git@github.com:gfx-rs/wgpu.git");
    }

    #[test]
    fn base_remote_uses_sole_remote() {
        let r = remotes(&[("weird", "git@github.com:a/b.git")]);
        assert_eq!(pick_base_remote(&r, None).unwrap().0, "weird");
    }

    #[test]
    fn base_remote_override_wins() {
        let r = remotes(&[("origin", "u1"), ("teoxoy", "u2")]);
        assert_eq!(pick_base_remote(&r, Some("teoxoy")).unwrap().0, "teoxoy");
    }

    #[test]
    fn base_remote_ambiguous_errors() {
        let r = remotes(&[("a", "u1"), ("b", "u2")]);
        assert!(pick_base_remote(&r, None).is_err());
        assert!(pick_base_remote(&r, Some("missing")).is_err());
    }
}
