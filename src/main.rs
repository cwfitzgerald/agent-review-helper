mod artifacts;
mod cli;
mod context;
mod gh;
mod jj;
mod layout;
mod manifest;
mod process;
mod range;
mod repo;
mod workspace;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::Path;

use cli::{Cli, Command, PrArgs, RangeArgs};
use context::ReviewContext;
use layout::Layout;

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    match cli.resolve()? {
        Command::Pr(args) => run_pr(args),
        Command::Range(args) => run_range(args),
    }
}

/// PR mode: fetch from GitHub, check out the head, gather diffs + conversation.
fn run_pr(args: PrArgs) -> Result<()> {
    let pr = args
        .pr
        .context("the `pr` command needs a PR number, e.g. `agent-review-helper pr 9464`")?;
    let cwd = std::env::current_dir().context("reading current directory")?;

    // 1. Discover the target repo + its GitHub identity.
    let target = repo::discover(&cwd, args.repo.as_deref(), args.remote.as_deref())?;
    let slug = target.origin.slug();
    step(&format!(
        "repo: {slug} via `{}` (root {})",
        target.remote,
        target.root.display()
    ));

    let bundle = format!("pr-{pr}");
    let ws_name = workspace::workspace_name(&bundle);
    let namespace = format!("{}-{}", target.origin.owner, target.origin.repo);
    let layout = Layout::resolve(args.storage, &target.root, &namespace, &bundle)?;

    // 1a. `--clean`: tear down this PR's workspace + artifacts and stop.
    if args.clean {
        return report_clean(&layout, &target.root, &ws_name, &format!("PR #{pr} ({slug})"));
    }

    // 2. Update trunk so `jj pr-diff`'s fork point is accurate.
    if args.no_fetch {
        step(&format!("skipping `{}` fetch (--no-fetch)", target.remote));
    } else {
        step(&format!("fetching `{}`", target.remote));
        jj::fetch(&target.root, &target.remote, None)?;
    }

    // 3. Resolve PR metadata.
    step(&format!("loading PR #{pr} metadata"));
    let pr_info = gh::pr_view(&slug, pr)?;

    // 4. Prepare folders.
    workspace::ensure_ignored(&target.root)?;
    workspace::prepare(&layout, &target.root, &ws_name, args.force)?;

    // 5. Set up the fork remote (if needed) and fetch the PR head branch.
    let revision = fetch_pr_head(&target, &pr_info)?;

    // 6. Create the PR workspace checkout.
    step(&format!("creating workspace at {}", layout.workspace.display()));
    jj::workspace_add(&target.root, &ws_name, &layout.workspace, &revision)?;

    // 7. Run all artifact producers.
    let ctx = ReviewContext {
        pr,
        repo: target.origin,
        repo_root: target.root,
        layout,
        pr_info,
    };

    let mut produced = Vec::new();
    for artifact in artifacts::default_registry() {
        step(&format!("gathering: {}", artifact.id()));
        produced.push(artifact.produce(&ctx)?);
    }

    // 8. Write the manifest that ties it together.
    manifest::write(&ctx, &produced)?;
    report_ready(&ctx.layout);
    Ok(())
}

/// Range mode: check out the target revision, gather the range diff + commit log.
/// Entirely local — no `gh`, no fetch, no remotes.
fn run_range(args: RangeArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("reading current directory")?;
    let root = jj::workspace_root(&cwd)
        .context("not inside a jj repository (run from your target repo)")?;
    let namespace = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".into());

    // Stable bundle id from the resolved endpoints, so `--clean` finds it again.
    let from_id = jj::commit_id(&root, &args.from)?;
    let to_id = jj::commit_id(&root, &args.to)?;
    let bundle = format!("range-{from_id}-{to_id}");
    let ws_name = workspace::workspace_name(&bundle);
    let layout = Layout::resolve(args.storage, &root, &namespace, &bundle)?;

    step(&format!(
        "range: `{}` -> `{}` (root {})",
        args.from,
        args.to,
        root.display()
    ));

    if args.clean {
        let label = format!("range `{}` -> `{}`", args.from, args.to);
        return report_clean(&layout, &root, &ws_name, &label);
    }

    workspace::ensure_ignored(&root)?;
    workspace::prepare(&layout, &root, &ws_name, args.force)?;

    step(&format!("creating workspace at {}", layout.workspace.display()));
    jj::workspace_add(&root, &ws_name, &layout.workspace, &args.to)?;

    step("gathering: range diff + commit log");
    let produced = range::gather(&root, &layout, &args.from, &args.to)?;

    manifest::write_range(&layout, &root, &args.from, &args.to, &produced)?;
    report_ready(&layout);
    Ok(())
}

/// Ensure the PR head is fetched into a remote bookmark and return the revset
/// that names it. Adds a fork remote (named after the fork owner) when the PR
/// comes from a different repo than origin.
fn fetch_pr_head(target: &repo::TargetRepo, pr: &gh::PrInfo) -> Result<String> {
    let base_owner = &target.origin.owner;
    match pr.head_owner() {
        Some(owner) if &owner != base_owner => {
            let fork_repo = pr.head_repo_name(&target.origin.repo);
            let url = target.origin.sibling_url(&owner, &fork_repo);
            if !jj::remote_exists(&target.root, &owner)? {
                step(&format!("adding fork remote `{owner}` -> {url}"));
                jj::remote_add(&target.root, &owner, &url)?;
            }
            step(&format!("fetching `{}` from `{owner}`", pr.head_ref_name));
            jj::fetch(&target.root, &owner, Some(&pr.head_ref_name))?;
            Ok(format!("{}@{}", pr.head_ref_name, owner))
        }
        _ => {
            // Same-repo PR (or unknown fork owner): the branch is on the base remote.
            step(&format!("fetching `{}` from `{}`", pr.head_ref_name, target.remote));
            jj::fetch(&target.root, &target.remote, Some(&pr.head_ref_name))?;
            Ok(format!("{}@{}", pr.head_ref_name, target.remote))
        }
    }
}

fn report_clean(layout: &Layout, root: &Path, ws_name: &str, label: &str) -> Result<()> {
    if workspace::clean(layout, root, ws_name)? {
        println!("Cleaned {label}.");
    } else {
        println!("Nothing to clean for {label}.");
    }
    Ok(())
}

fn report_ready(layout: &Layout) {
    println!();
    println!("Review bundle ready:");
    println!("  info:      {}", layout.info.display());
    println!("  workspace: {}", layout.workspace.display());
}

fn step(msg: &str) {
    println!("• {msg}");
}
