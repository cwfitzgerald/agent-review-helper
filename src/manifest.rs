//! The `README.md` written into the info folder: a map of what's here and how
//! to feed it to a review skill, so the LLM gets oriented with zero extra calls.

use anyhow::{Context, Result};
use std::fmt::Write as _;
use std::fs;

use crate::artifacts::Produced;
use crate::context::ReviewContext;

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

    writeln!(out, "## Building / testing in the workspace").ok();
    writeln!(out).ok();
    writeln!(
        out,
        "When you run cargo in the workspace, set `CARGO_TARGET_DIR` to the root repo's \
         target dir so dependencies aren't recompiled from scratch:"
    )
    .ok();
    writeln!(out).ok();
    let target = ctx.shared_target_dir();
    writeln!(out, "```").ok();
    writeln!(out, "CARGO_TARGET_DIR={}", target.display()).ok();
    writeln!(out, "```").ok();
    writeln!(out).ok();
    writeln!(
        out,
        "Set it on every build/test/clippy invocation (PowerShell: \
         `$env:CARGO_TARGET_DIR='{}'`).",
        target.display()
    )
    .ok();

    let file = ctx.layout.info.join("README.md");
    fs::write(&file, out).with_context(|| format!("writing {}", file.display()))?;
    Ok(())
}
