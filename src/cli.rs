//! Command-line surface.

use clap::Parser;

use crate::layout::Storage;

/// Prepare a self-contained folder of PR review artifacts for an LLM.
///
/// Run from inside your target repo (e.g. a wgpu checkout). Creates a jj
/// workspace at the PR head and gathers diffs + the full conversation.
#[derive(Debug, Parser)]
#[command(name = "agent-review-helper", version)]
pub struct Cli {
    /// PR number to prepare (e.g. `agent-review-helper 8967`).
    pub pr: u64,

    /// Recreate artifacts and workspace if they already exist.
    #[arg(long)]
    pub force: bool,

    /// Remove this PR's workspace + artifacts (and forget the jj workspace), then exit.
    #[arg(long)]
    pub clean: bool,

    /// Skip `jj git fetch` of the base remote (use already-fetched state).
    #[arg(long)]
    pub no_fetch: bool,

    /// Name of the base remote (the PR's upstream). Defaults to `origin`, then
    /// `upstream`, then the sole remote if there is exactly one.
    #[arg(long)]
    pub remote: Option<String>,

    /// Override the GitHub repo as `owner/repo` instead of deriving from the base remote.
    #[arg(long)]
    pub repo: Option<String>,

    /// Where to store the workspace + artifacts.
    #[arg(long, value_enum, default_value_t = Storage::InRepo)]
    pub storage: Storage,
}
