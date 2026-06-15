//! `gh` command wrappers and the JSON models we deserialize from them.
//!
//! Note the two key-casing worlds, which the models below are annotated for:
//! - `gh pr view --json` returns **camelCase** keys (GraphQL-backed).
//! - `gh api` (REST) returns **snake_case** keys.

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::process::run;

const PR_VIEW_FIELDS: &str = "number,title,state,body,author,baseRefName,headRefName,\
headRepository,headRepositoryOwner,url,createdAt,isDraft,additions,deletions,changedFiles,\
mergeable,labels";

#[derive(Debug, Deserialize)]
pub struct Author {
    #[serde(default)]
    pub login: String,
}

#[derive(Debug, Deserialize)]
pub struct NamedRepo {
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Label {
    #[serde(default)]
    pub name: String,
}

/// PR metadata from `gh pr view --json` (camelCase).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrInfo {
    pub number: u64,
    pub title: String,
    pub state: String,
    #[serde(default)]
    pub body: String,
    pub author: Author,
    pub base_ref_name: String,
    pub head_ref_name: String,
    pub head_repository: Option<NamedRepo>,
    pub head_repository_owner: Option<Author>,
    pub url: String,
    pub created_at: String,
    #[serde(default)]
    pub is_draft: bool,
    #[serde(default)]
    pub additions: u64,
    #[serde(default)]
    pub deletions: u64,
    #[serde(default)]
    pub changed_files: u64,
    #[serde(default)]
    pub mergeable: String,
    #[serde(default)]
    pub labels: Vec<Label>,
}

impl PrInfo {
    /// The head repo name (defaults to the base repo name if absent).
    pub fn head_repo_name(&self, fallback: &str) -> String {
        self.head_repository
            .as_ref()
            .map(|r| r.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| fallback.to_string())
    }

    /// The login of the head repo owner, if known.
    pub fn head_owner(&self) -> Option<String> {
        self.head_repository_owner
            .as_ref()
            .map(|o| o.login.clone())
            .filter(|l| !l.is_empty())
    }
}

/// A top-level issue comment from `gh api .../issues/<n>/comments` (snake_case).
#[derive(Debug, Deserialize)]
pub struct IssueComment {
    pub user: Author,
    #[serde(default)]
    pub body: String,
    pub created_at: String,
    #[serde(default)]
    pub html_url: String,
}

/// A review submission from `gh api .../pulls/<n>/reviews` (snake_case).
#[derive(Debug, Deserialize)]
pub struct Review {
    pub user: Author,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub state: String,
    /// `null` for pending reviews; we skip those.
    pub submitted_at: Option<String>,
    #[serde(default)]
    pub html_url: String,
}

/// An inline (code-anchored) review comment from `gh api .../pulls/<n>/comments`.
#[derive(Debug, Deserialize)]
pub struct ReviewComment {
    pub user: Author,
    #[serde(default)]
    pub body: String,
    pub created_at: String,
    #[serde(default)]
    pub path: String,
    pub line: Option<u64>,
    pub original_line: Option<u64>,
    #[serde(default)]
    pub diff_hunk: String,
    pub in_reply_to_id: Option<u64>,
    #[serde(default)]
    pub html_url: String,
}

/// `gh pr view <n> --repo <slug> --json ...`
pub fn pr_view(slug: &str, pr: u64) -> Result<PrInfo> {
    let pr_s = pr.to_string();
    let json = run(
        "gh",
        &["pr", "view", &pr_s, "--repo", slug, "--json", PR_VIEW_FIELDS],
        None,
    )?;
    serde_json::from_str(&json).context("parsing `gh pr view` JSON")
}

/// `gh pr diff <n> --repo <slug>` — GitHub's view of the diff.
pub fn pr_diff(slug: &str, pr: u64) -> Result<String> {
    let pr_s = pr.to_string();
    run("gh", &["pr", "diff", &pr_s, "--repo", slug], None)
}

/// Fetch + deserialize a paginated REST array endpoint.
fn api_list<T: for<'de> Deserialize<'de>>(slug: &str, path_suffix: &str) -> Result<Vec<T>> {
    let endpoint = format!("repos/{slug}/{path_suffix}");
    let json = run("gh", &["api", "--paginate", &endpoint], None)
        .with_context(|| format!("GET {endpoint}"))?;
    serde_json::from_str(&json).with_context(|| format!("parsing JSON from {endpoint}"))
}

pub fn issue_comments(slug: &str, pr: u64) -> Result<Vec<IssueComment>> {
    api_list(slug, &format!("issues/{pr}/comments"))
}

pub fn reviews(slug: &str, pr: u64) -> Result<Vec<Review>> {
    api_list(slug, &format!("pulls/{pr}/reviews"))
}

pub fn review_comments(slug: &str, pr: u64) -> Result<Vec<ReviewComment>> {
    api_list(slug, &format!("pulls/{pr}/comments"))
}
