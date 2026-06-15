---
name: agent-review
description: >
  Locally review a GitHub PR by number using the agent-review-helper tool.
  Use when the user asks to review / onboard to a specific PR number in the
  current repo (e.g. "review 8967", "let's look at PR #8967", "onboard me to
  8967"), especially in wgpu or other jj-managed Rust repos. This skill runs
  the agent-review-helper binary to gather a self-contained review bundle
  (diffs + full chronological conversation + a checked-out workspace), then
  reviews from that bundle. Prefer this over ad-hoc gh/jj calls when a PR
  number is given.
---

# Local PR Review via agent-review-helper

## Purpose

`agent-review-helper` is a Rust CLI that prepares everything needed to review a
PR — with **no extra unneeded context** — and checks the PR out into a jj
workspace. This skill runs it, then performs an adversarial, high-detail review
from the produced bundle. The user is a graphics engineer working mainly on
wgpu/GPU Rust; assume familiarity with graphics pipelines, GPU APIs, async, and
unsafe Rust.

## Step 1 — Gather the bundle

Run the tool from the repo the user is in (the working directory is already the
target repo). `$ARGUMENTS` is the PR number.

```
agent-review-helper <PR>
```

- It prints `info:` and `workspace:` paths on success — note them.
- If it reports the bundle already exists and the user wants a fresh pull, re-run
  with `--force`. Otherwise reuse the existing bundle (do not re-run).
- It uses read-only `gh` and `jj git fetch` / `jj workspace add`; it does **not**
  commit anything. If it fails, surface the exact error — do not fall back to
  manual gathering without telling the user.
- **If the command is not found**, install it (see "Installing / updating" below),
  then retry.

The bundle (default `.agent-review/pr-<n>/info/`) contains:

- `README.md` — manifest: PR title/state, the workspace path, and what each file is.
- `conversation.md` — chronological PR description, comments, reviews, and inline
  threads (with `file:line` + diff hunks). **Read this fully** — authors hide
  critical reasoning in inline comments.
- `pr-diff.diff` — the local merge-base diff (`jj pr-diff`). This is the primary
  diff to review.
- `upstream.diff` — GitHub's diff, for cross-checking if the local one looks off.
- The full PR is checked out at the `workspace:` path for building, running
  tests, or navigating surrounding code.

**Building / testing in the workspace:** always set `CARGO_TARGET_DIR` to the
root repo's `target/` dir so cargo reuses already-built dependencies instead of
recompiling everything. The exact value is in the bundle `README.md` under
"Building / testing in the workspace". Set it on every `cargo` /
`cargo nextest` / `cargo clippy` call — e.g. PowerShell
`$env:CARGO_TARGET_DIR='<root>/target'; cargo nextest run`, or bash
`CARGO_TARGET_DIR=<root>/target cargo build`.

## Step 2 — Review

Read `README.md`, then `conversation.md`, then `pr-diff.diff`. Read from these
files — do **not** re-issue `gh`/`jj` calls for data already in the bundle. For
large diffs, spawn **foreground** sub-agents split by subsystem or concern.

Then present, in this order:

1. **Goal & motivation** — what the change accomplishes and why.
2. **Architecture / flow** — execution path or structural reorganization; diagram
   if complex.
3. **Key decisions** — design choices, alternatives, anything chosen for
   convenience over correctness.
4. **Risk areas** — likely bugs, edge cases, regressions, maintenance burdens. Be
   adversarial; assume problems exist and find them.
5. **Testing** — what's tested, what's not, whether tests verify the invariants
   that matter.
6. **Commentary summary** — what was debated in the conversation; what's unresolved.
7. **Review order** (mandatory) — an ordered reading plan (commit-by-commit if the
   PR is structured that way, otherwise file-by-file) with a one-line rationale each.

## Step 3 — Interactive

Ask the user about their focus and desired depth before and during. Be direct;
don't hedge. When they raise a concern, help phrase it precisely for a review
comment. Over-communicate and over-question — the user wants to stay in control.

## Installing / updating

The binary is built from <https://github.com/cwfitzgerald/agent-review-helper>.

```
# Install (or reinstall) the latest version:
cargo install --git https://github.com/cwfitzgerald/agent-review-helper --locked --force
```

`cargo install` places it in `~/.cargo/bin`, which must be on PATH. Run the same
command to update to the latest after the tool changes.

## Notes

- The tool needs to run inside a jj repo whose `origin` points at the PR's repo.
  If the user is in the wrong directory, say so.
- To inspect a different PR, just run the tool again with that number.
