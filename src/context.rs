//! The resolved context handed to every artifact producer.

use crate::gh::PrInfo;
use crate::layout::Layout;
use crate::repo::RepoRef;

/// Everything an [`crate::artifacts::Artifact`] needs to do its job. Built once
/// during orchestration, then shared (read-only) across producers.
pub struct ReviewContext {
    pub pr: u64,
    /// GitHub repo the PR lives in (the base repo).
    pub repo: RepoRef,
    /// Resolved on-disk locations for this PR.
    pub layout: Layout,
    /// PR metadata from `gh pr view`.
    pub pr_info: PrInfo,
}

impl ReviewContext {
    pub fn slug(&self) -> String {
        self.repo.slug()
    }
}
