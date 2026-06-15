//! The resolved context handed to every artifact producer.

use crate::gh::PrInfo;
use crate::layout::Layout;
use crate::repo::RepoRef;
use std::path::PathBuf;

/// Everything an [`crate::artifacts::Artifact`] needs to do its job. Built once
/// during orchestration, then shared (read-only) across producers.
pub struct ReviewContext {
    pub pr: u64,
    /// GitHub repo the PR lives in (the base repo).
    pub repo: RepoRef,
    /// Root of the target jj workspace we were invoked from. Used to point the
    /// PR workspace's cargo builds at the root repo's `target/` dir.
    pub repo_root: PathBuf,
    /// Resolved on-disk locations for this PR.
    pub layout: Layout,
    /// PR metadata from `gh pr view`.
    pub pr_info: PrInfo,
}

impl ReviewContext {
    pub fn slug(&self) -> String {
        self.repo.slug()
    }

    /// The root repo's cargo target directory. Builds in the PR workspace
    /// should point `CARGO_TARGET_DIR` here to reuse compiled dependencies
    /// instead of recompiling them from scratch.
    pub fn shared_target_dir(&self) -> PathBuf {
        self.repo_root.join("target")
    }
}
