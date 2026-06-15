//! Artifact producers.
//!
//! Each gather step implements [`Artifact`]. Orchestration runs a registry of
//! them, so extending the tool with new artifacts (e.g. CI logs, related
//! issues, a call graph) means adding a producer here — nothing else changes.

use anyhow::Result;
use std::path::PathBuf;

use crate::context::ReviewContext;

mod conversation;
mod diff;

pub use conversation::ConversationArtifact;
pub use diff::{PrDiffArtifact, UpstreamDiffArtifact};

/// One produced file plus a short human description for the manifest.
pub struct Produced {
    pub file: PathBuf,
    pub description: String,
}

/// A single gather step that writes one (or more) files into `ctx.layout.info`.
pub trait Artifact {
    /// Stable identifier, used in logs.
    fn id(&self) -> &'static str;
    /// Produce the artifact, returning what was written for the manifest.
    fn produce(&self, ctx: &ReviewContext) -> Result<Produced>;
}

/// The default set of artifacts, in production order.
pub fn default_registry() -> Vec<Box<dyn Artifact>> {
    vec![
        Box::new(PrDiffArtifact),
        Box::new(UpstreamDiffArtifact),
        Box::new(ConversationArtifact),
    ]
}
