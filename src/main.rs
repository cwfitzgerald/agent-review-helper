mod artifacts;
mod cli;
mod context;
mod gh;
mod jj;
mod layout;
mod manifest;
mod process;
mod repo;
mod workspace;

use anyhow::{Context, Result};
use clap::Parser;

use cli::Cli;
use context::ReviewContext;
use layout::Layout;

fn main() -> Result<()> {
    let args = Cli::parse();
    if let Err(err) = run(&args) {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
    Ok(())
}

fn run(args: &Cli) -> Result<()> {
    let cwd = std::env::current_dir().context("reading current directory")?;

    // 1. Discover the target repo + its GitHub identity.
    let target = repo::discover(&cwd, args.repo.as_deref(), args.remote.as_deref())?;
    let slug = target.origin.slug();
    step(&format!(
        "repo: {slug} via `{}` (root {})",
        target.remote,
        target.root.display()
    ));

    // 1a. `--clean`: tear down this PR's workspace + artifacts and stop.
    if args.clean {
        let layout = Layout::resolve(args.storage, &target.root, &target.origin, args.pr)?;
        if workspace::clean(&layout, &target.root, args.pr)? {
            println!("Cleaned PR #{} ({}).", args.pr, slug);
        } else {
            println!("Nothing to clean for PR #{} ({}).", args.pr, slug);
        }
        return Ok(());
    }

    // 2. Update trunk so `jj pr-diff`'s fork point is accurate.
    if args.no_fetch {
        step(&format!("skipping `{}` fetch (--no-fetch)", target.remote));
    } else {
        step(&format!("fetching `{}`", target.remote));
        jj::fetch(&target.root, &target.remote, None)?;
    }

    // 3. Resolve PR metadata.
    step(&format!("loading PR #{} metadata", args.pr));
    let pr_info = gh::pr_view(&slug, args.pr)?;

    // 4. Resolve on-disk layout and prepare folders.
    let layout = Layout::resolve(args.storage, &target.root, &target.origin, args.pr)?;
    workspace::ensure_ignored(&target.root)?;
    workspace::prepare(&layout, &target.root, args.pr, args.force)?;

    // 5. Set up the fork remote (if needed) and fetch the PR head branch.
    let revision = fetch_pr_head(&target, &pr_info)?;

    // 6. Create the PR workspace checkout.
    let ws_name = workspace::workspace_name(args.pr);
    step(&format!("creating workspace at {}", layout.workspace.display()));
    jj::workspace_add(&target.root, &ws_name, &layout.workspace, &revision)?;

    // 7. Run all artifact producers.
    let ctx = ReviewContext {
        pr: args.pr,
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

    println!();
    println!("Review bundle ready:");
    println!("  info:      {}", ctx.layout.info.display());
    println!("  workspace: {}", ctx.layout.workspace.display());
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

fn step(msg: &str) {
    println!("• {msg}");
}
