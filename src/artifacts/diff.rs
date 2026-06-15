//! Diff artifacts: the local merge-base diff (`jj pr-diff`) and GitHub's view
//! (`gh pr diff`). Both are kept so the reviewer can cross-check them.

use anyhow::Result;
use std::fs;

use super::{Artifact, Produced};
use crate::context::ReviewContext;
use crate::{gh, jj};

/// `jj pr-diff` run inside the PR workspace: `fork_point(@ | trunk())..@`.
pub struct PrDiffArtifact;

impl Artifact for PrDiffArtifact {
    fn id(&self) -> &'static str {
        "pr-diff"
    }

    fn produce(&self, ctx: &ReviewContext) -> Result<Produced> {
        let diff = jj::pr_diff(&ctx.layout.workspace)?;
        let file = ctx.layout.info.join("pr-diff.diff");
        fs::write(&file, diff)?;
        Ok(Produced {
            file,
            description: "Local diff against the PR's fork point (`jj pr-diff`, git format)."
                .into(),
        })
    }
}

/// `gh pr diff` — GitHub's computed diff for the PR.
pub struct UpstreamDiffArtifact;

impl Artifact for UpstreamDiffArtifact {
    fn id(&self) -> &'static str {
        "upstream-diff"
    }

    fn produce(&self, ctx: &ReviewContext) -> Result<Produced> {
        let diff = gh::pr_diff(&ctx.slug(), ctx.pr)?;
        let file = ctx.layout.info.join("upstream.diff");
        fs::write(&file, diff)?;
        Ok(Produced {
            file,
            description: "GitHub's view of the PR diff (`gh pr diff`), for cross-checking."
                .into(),
        })
    }
}
