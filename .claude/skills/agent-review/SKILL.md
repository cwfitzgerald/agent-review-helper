---
name: agent-review
description: >
  Locally review a GitHub PR by number, or a local jj revision range, using the
  agent-review-helper tool. Use only when the user explicitly invokes or names
  the `agent-review` skill (for example, `$agent-review`, `/agent-review`, or
  "use the agent-review skill"). Do not trigger for generic requests to review
  or onboard to a PR, changeset, revision range, or working-copy changes. This
  skill gathers a self-contained review bundle (diffs + conversation for PRs +
  a checked-out workspace), then reviews from that bundle.
---

# Local PR Review via agent-review-helper

## Purpose

`agent-review-helper` is a Rust CLI that prepares everything needed to review a
PR — with **no extra unneeded context** — and checks the PR out into a jj
workspace. This skill runs it, then performs an adversarial, high-detail review
from the produced bundle. The user is a graphics engineer working mainly on
wgpu/GPU Rust; assume familiarity with graphics pipelines, GPU APIs, async, and
unsafe Rust.

The goal is not a summary — it is to make the user **ready to comment on the
PR**. Every review should leave the user able to (a) understand the change
deeply enough to defend or challenge it, and (b) raise precise, well-supported
concerns. Bias toward thoroughness over speed: read every changed file, follow
the code into the surrounding workspace, and verify claims rather than trusting
them. A shallow-but-fast review is a failure even if it sounds confident.

## Step 1 — Gather the bundle

Run the tool from the repo the user is in (the working directory is already the
target repo).

**PR review** (`$ARGUMENTS` is the PR number):

```
agent-review-helper <PR>
```

**Local revision range** (no GitHub — for reviewing uncommitted/unmerged local
work or any `from..to` span). Map the user's request to `-f`/`-t`: e.g. "review
my changes" → `-f trunk -t @`; "review this PR's worth of work" → the bookmark
range they name. `-t` defaults to `@`.

```
agent-review-helper range -f <from> -t <to>
```

This bundle (default `.agent-review/range-<from>-<to>/info/`) has `range.diff`,
`commits.txt` (the range's commit log — use it for the review order), the
manifest `README.md`, and a workspace checked out at `<to>`. There is **no**
`conversation.md` / `upstream.diff` (no PR). Skip those steps below for ranges.

- It prints `info:` and `workspace:` paths on success — note them.
- If it reports the bundle already exists and the user wants a fresh pull, re-run
  with `--force`. Otherwise reuse the existing bundle (do not re-run).
- It auto-detects the PR's base remote (`origin`, else `upstream`, else the sole
  remote). If it reports the base remote is ambiguous, re-run with
  `--remote <name>` naming the upstream remote (e.g. `--remote upstream`).
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

Read `README.md`, then `conversation.md`, then `pr-diff.diff` (for a range:
`README.md` → `commits.txt` → `range.diff`). Read from these files — do **not**
re-issue `gh`/`jj` calls for data already in the bundle. For large diffs, spawn
**foreground** sub-agents split by subsystem or concern.

Be thorough: read **every** changed file in full, not just the diff hunks — a
hunk rarely tells you whether the surrounding code still holds. Follow callers
and callees into the workspace to understand how the change behaves in context.
Do not present a finding you have not grounded in the actual code.

### Validate every bug before presenting it (mandatory)

You may **not** present a suspected bug, regression, or correctness concern until
you have validated it against the real code in the checked-out workspace. For
each candidate finding:

1. Open the actual file(s) in the `workspace:` path and trace the real control
   flow / types / lifetimes — confirm the bug is reachable and the conditions
   that trigger it genuinely hold. Many "bugs" evaporate once you read the code
   around the hunk.
2. Where feasible, **prove it**: write or point to a failing test, construct a
   concrete repro, or build/run the relevant code (`cargo nextest` / `cargo
   build` / `cargo clippy`, with `CARGO_TARGET_DIR` set as described above). Use
   foreground sub-agents to parallelize verification across findings.
3. Only findings you have confirmed make it into the **Risk areas** list. If a
   concern is plausible but you could not confirm it, either drop it or list it
   **separately and explicitly labeled `unverified`**, with what would be needed
   to confirm. Never let an unverified hunch masquerade as a confirmed bug.

State *how* each confirmed bug was validated (code path traced, test run, repro
built) so the user can trust it when commenting.

Then present, in this order:

1. **Goal & motivation** — what the change accomplishes and why.
2. **Architecture / flow** — execution path or structural reorganization; diagram
   if complex.
3. **Key decisions** — design choices, alternatives, anything chosen for
   convenience over correctness.
4. **Risk areas** — confirmed bugs, edge cases, regressions, maintenance burdens.
   Be adversarial; assume problems exist and find them. Every item here must have
   passed the validation gate above — note how each was confirmed. List any
   plausible-but-unconfirmed concerns in a separate `unverified` subsection.
5. **Testing** — what's tested, what's not, whether tests verify the invariants
   that matter.
6. **Commentary summary** — what was debated in the conversation; what's unresolved.
   (PR only — skip for a range, which has no conversation.)
7. **Review order** (mandatory) — an ordered reading plan (commit-by-commit if the
   PR is structured that way, otherwise file-by-file) with a one-line rationale each.

## Step 3 — Interactive & comment prep

**Always deliver the complete report first.** Do **not** open by asking "where do
you want to focus?" or otherwise gate the review on the user's input — they want
the full, thorough report covering every section above, with all bugs already
validated, before any back-and-forth. Review the whole change; never scope it
down preemptively. Be direct; don't hedge.

Only **after** the complete report is on the table, go interactive to drive
toward PR comments. The end goal is comments on the PR. Actively help the user
get there:

- For each confirmed risk area, offer a **ready-to-post comment**: the precise
  `file:line` anchor (from the bundle), a crisp statement of the problem, the
  evidence that validated it, and a concrete suggested fix or question for the
  author. Phrase it as the user would post it, not as a description of the issue.
- When the user raises their own concern, help sharpen it the same way — anchor,
  evidence, suggestion — and validate it against the code before wording it.
- Distinguish blocking issues from nits/suggestions so the user knows what to
  insist on versus mention.
- Surface the open questions worth asking the author (ambiguities, missing tests,
  unstated assumptions) as draft comment text.

## Installing / updating

The binary is built from <https://github.com/cwfitzgerald/agent-review-helper>.

```
# Install (or reinstall) the latest version:
cargo install --git https://github.com/cwfitzgerald/agent-review-helper --locked --force
```

`cargo install` places it in `~/.cargo/bin`, which must be on PATH. Run the same
command to update to the latest after the tool changes.

## Cleaning up

When the user is done, tear down the workspace + artifacts (pass the same
arguments plus `--clean`):

```
agent-review-helper <PR> --clean
agent-review-helper range -f <from> -t <to> --clean
```

This forgets the jj workspace and removes the bundle folder. It does no network
or `gh` calls and is idempotent (a second `--clean` is a no-op). Fork remotes the
tool added are left in place (harmless and reused on future runs).

## Notes

- PR mode needs a jj repo with a remote pointing at the PR's base repo (`origin`
  or `upstream`). Range mode is fully local — any jj repo works. If the user is
  in the wrong directory, say so.
- To inspect a different PR or range, just run the tool again with new arguments.
