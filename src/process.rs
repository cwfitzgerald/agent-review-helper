//! Thin wrappers around child processes.
//!
//! Every external interaction in this tool goes through [`run`] (capture stdout)
//! or [`run_status`] (run for side effects). Keeping process handling in one
//! place means error reporting, working-directory handling, and logging stay
//! consistent as we add more commands.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// Run `program args...` (optionally in `cwd`) and return captured stdout.
///
/// Fails with the program's stderr attached if the exit status is non-zero.
pub fn run(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let out = build(program, args, cwd)
        .output()
        .with_context(|| format!("failed to spawn `{program}` (is it on PATH?)"))?;

    if !out.status.success() {
        bail!(
            "`{} {}` failed ({})\nstderr:\n{}",
            program,
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run `program args...` for its side effects, discarding stdout.
pub fn run_status(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<()> {
    run(program, args, cwd).map(|_| ())
}

fn build(program: &str, args: &[&str], cwd: Option<&Path>) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd
}
