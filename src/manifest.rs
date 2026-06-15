//! The `README.md` written into the info folder: a map of what's here and how
//! to feed it to a review skill, so the LLM gets oriented with zero extra calls.

use anyhow::{Context, Result};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::artifacts::Produced;
use crate::context::ReviewContext;
use crate::layout::Layout;

/// PR bundle manifest.
pub fn write(ctx: &ReviewContext, produced: &[Produced]) -> Result<()> {
    let pr = &ctx.pr_info;
    let mut out = String::new();

    writeln!(out, "# Review bundle — PR #{} ({})", pr.number, ctx.slug()).ok();
    writeln!(out).ok();
    writeln!(out, "> {}", pr.title).ok();
    writeln!(out).ok();
    writeln!(out, "- **State**: {}{}", pr.state, if pr.is_draft { " (draft)" } else { "" }).ok();
    writeln!(out, "- **Author**: @{}", pr.author.login).ok();
    writeln!(out, "- **URL**: {}", pr.url).ok();
    writeln!(out, "- **Workspace** (full checkout): `{}`", ctx.layout.workspace.display()).ok();
    writeln!(out).ok();

    artifacts_section(&mut out, produced);

    writeln!(out, "## How to review").ok();
    writeln!(out).ok();
    writeln!(
        out,
        "All context needed to review this PR is in this folder. Read `conversation.md` \
         for intent and discussion, then `pr-diff.diff` for the change. The full PR is \
         checked out at the workspace path above if you need to build, run tests, or \
         navigate surrounding code. Cross-check `upstream.diff` if the local diff looks \
         off (e.g. trunk moved)."
    )
    .ok();
    writeln!(out).ok();

    building_section(&mut out, &ctx.shared_target_dir());

    let file = ctx.layout.info.join("README.md");
    fs::write(&file, out).with_context(|| format!("writing {}", file.display()))?;
    Ok(())
}

/// Range bundle manifest. `repo_root` is the parent repo whose `target/` dir the
/// workspace should share for builds.
pub fn write_range(
    layout: &Layout,
    repo_root: &Path,
    from: &str,
    to: &str,
    produced: &[Produced],
) -> Result<()> {
    let mut out = String::new();

    writeln!(out, "# Review bundle — range `{from}` → `{to}`").ok();
    writeln!(out).ok();
    writeln!(out, "- **From** (base): `{from}`").ok();
    writeln!(out, "- **To** (target): `{to}`").ok();
    writeln!(out, "- **Workspace** (checkout at target): `{}`", layout.workspace.display()).ok();
    writeln!(out).ok();

    artifacts_section(&mut out, produced);

    writeln!(out, "## How to review").ok();
    writeln!(out).ok();
    writeln!(
        out,
        "This is a local revision range, not a GitHub PR — there is no upstream \
         conversation. Read `commits.txt` for the commit structure, then `range.diff` \
         for the change. The target revision is checked out at the workspace path above \
         if you need to build, run tests, or navigate surrounding code."
    )
    .ok();
    writeln!(out).ok();

    building_section(&mut out, &repo_root.join("target"));

    let file = layout.info.join("README.md");
    fs::write(&file, out).with_context(|| format!("writing {}", file.display()))?;
    Ok(())
}

fn artifacts_section(out: &mut String, produced: &[Produced]) {
    writeln!(out, "## Artifacts").ok();
    writeln!(out).ok();
    for p in produced {
        let name = p
            .file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        writeln!(out, "- `{}` — {}", name, p.description).ok();
    }
    writeln!(out).ok();
}

fn building_section(out: &mut String, target_dir: &Path) {
    writeln!(out, "## Building / testing in the workspace").ok();
    writeln!(out).ok();
    writeln!(
        out,
        "When you run cargo in the workspace, set `CARGO_TARGET_DIR` to the root repo's \
         target dir so dependencies aren't recompiled from scratch:"
    )
    .ok();
    writeln!(out).ok();
    writeln!(out, "```").ok();
    writeln!(out, "CARGO_TARGET_DIR={}", target_dir.display()).ok();
    writeln!(out, "```").ok();
    writeln!(out).ok();
    writeln!(
        out,
        "Set it on every build/test/clippy invocation (PowerShell: \
         `$env:CARGO_TARGET_DIR='{}'`).",
        target_dir.display()
    )
    .ok();
}
