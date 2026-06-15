//! Range mode: gather a local jj revision range into a bundle. Mirrors the PR
//! artifact set, minus everything GitHub-specific (no `gh`, no conversation).

use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::artifacts::Produced;
use crate::jj;
use crate::layout::Layout;

/// Write the range diff + commit log into `layout.info`, returning the manifest
/// entries.
pub fn gather(root: &Path, layout: &Layout, from: &str, to: &str) -> Result<Vec<Produced>> {
    let diff = jj::diff_range(root, from, to)?;
    let diff_file = layout.info.join("range.diff");
    fs::write(&diff_file, diff)?;

    let log = jj::log_range(root, from, to)?;
    let log_file = layout.info.join("commits.txt");
    fs::write(&log_file, log)?;

    Ok(vec![
        Produced {
            file: diff_file,
            description: format!("Diff of the range (`jj diff -f {from} -t {to}`, git format)."),
        },
        Produced {
            file: log_file,
            description: format!("Commit log of the range (`jj log -r {from}..{to}`)."),
        },
    ])
}
