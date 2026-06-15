//! Command-line surface.
//!
//! Two modes share the same bundle machinery: `pr <PR>` (GitHub PR review) and
//! `range -f <from> -t <to>` (a local jj revision range). A bare PR number is a
//! backwards-compatible shorthand for `pr <PR>`, so `agent-review-helper 9464`
//! keeps working.

use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};

use crate::layout::Storage;

/// Prepare a self-contained folder of review artifacts for an LLM.
///
/// Run from inside your target repo (e.g. a wgpu checkout). Creates a jj
/// workspace at the change and gathers diffs (+ the full conversation for PRs).
#[derive(Debug, Parser)]
#[command(name = "agent-review-helper", version, args_conflicts_with_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Backwards-compatible shorthand: a bare PR number runs `pr <PR>`.
    #[command(flatten)]
    pub pr: PrArgs,
}

impl Cli {
    /// Resolve the effective command, defaulting a bare PR number to `pr`.
    pub fn resolve(self) -> Result<Command> {
        match self.command {
            Some(cmd) => Ok(cmd),
            None if self.pr.pr.is_some() => Ok(Command::Pr(self.pr)),
            None => bail!(
                "provide a PR number (e.g. `agent-review-helper 9464`) or a subcommand \
                 (`pr <PR>`, `range -f <from> -t <to>`)"
            ),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Prepare a review bundle for a GitHub PR.
    Pr(PrArgs),
    /// Prepare a review bundle for a local jj revision range.
    Range(RangeArgs),
}

#[derive(Debug, Args)]
pub struct PrArgs {
    /// PR number to prepare (e.g. `agent-review-helper 8967`).
    pub pr: Option<u64>,

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

#[derive(Debug, Args)]
pub struct RangeArgs {
    /// Base revision (`-f`/from): the diff and log start after this revision.
    #[arg(short = 'f', long)]
    pub from: String,

    /// Target revision (`-t`/to). Defaults to the working copy `@`.
    #[arg(short = 't', long, default_value = "@")]
    pub to: String,

    /// Recreate artifacts and workspace if they already exist.
    #[arg(long)]
    pub force: bool,

    /// Remove this range's workspace + artifacts (and forget the jj workspace), then exit.
    #[arg(long)]
    pub clean: bool,

    /// Where to store the workspace + artifacts.
    #[arg(long, value_enum, default_value_t = Storage::InRepo)]
    pub storage: Storage,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> Result<Command> {
        Cli::try_parse_from(std::iter::once("arh").chain(args.iter().copied()))
            .map_err(anyhow::Error::from)?
            .resolve()
    }

    #[test]
    fn bare_number_is_pr() {
        let cmd = parse(&["9464"]).unwrap();
        let Command::Pr(args) = cmd else { panic!("expected Pr") };
        assert_eq!(args.pr, Some(9464));
    }

    #[test]
    fn bare_number_with_flags() {
        let Command::Pr(args) = parse(&["9464", "--force", "--remote", "upstream"]).unwrap() else {
            panic!("expected Pr")
        };
        assert_eq!(args.pr, Some(9464));
        assert!(args.force);
        assert_eq!(args.remote.as_deref(), Some("upstream"));
    }

    #[test]
    fn explicit_pr_subcommand() {
        let Command::Pr(args) = parse(&["pr", "8967"]).unwrap() else { panic!("expected Pr") };
        assert_eq!(args.pr, Some(8967));
    }

    #[test]
    fn range_subcommand() {
        let Command::Range(args) = parse(&["range", "-f", "trunk", "-t", "@"]).unwrap() else {
            panic!("expected Range")
        };
        assert_eq!(args.from, "trunk");
        assert_eq!(args.to, "@");
    }

    #[test]
    fn range_to_defaults_to_working_copy() {
        let Command::Range(args) = parse(&["range", "-f", "trunk"]).unwrap() else {
            panic!("expected Range")
        };
        assert_eq!(args.to, "@");
    }

    #[test]
    fn no_args_errors() {
        assert!(parse(&[]).is_err());
    }
}
