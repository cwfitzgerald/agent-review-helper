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
workspace. This skill runs it, then reviews the change from the produced bundle.

This is a review, not an onboarding or a summary. Output is findings: defects,
unsupported claims, and machine-generated slop, each grounded in the actual
code. Explanation of the change earns only a short preamble; everything after it
must be something wrong or something the reader has to decide about. Bias toward
thoroughness over speed — read every changed file, follow the code into the
surrounding workspace, verify rather than trust. A shallow-but-fast review is a
failure even if it sounds confident.

The reader is a graphics engineer working mainly on wgpu/GPU Rust. Assume
fluency in graphics pipelines, GPU APIs, async, and unsafe Rust; never explain
language or domain basics.

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

## Step 2 — Read

Read `README.md`, then `conversation.md`, then `pr-diff.diff` (for a range:
`README.md` → `commits.txt` → `range.diff`). Read from these files — do **not**
re-issue `gh`/`jj` calls for data already in the bundle. For large diffs, spawn
**foreground** sub-agents split by subsystem or concern.

Read **every** changed file in full, not just the diff hunks — a hunk rarely
tells you whether the surrounding code still holds. Follow callers and callees
into the workspace. Never present a finding you have not grounded in the actual
code.

## Step 3 — What to hunt for

Assume the change is wrong somewhere and go find it. Three lenses, applied to
every changed file.

### Correctness

Real defects, in roughly descending order of what actually bites: wrong control
flow, off-by-one and boundary handling, integer/float conversion and overflow,
error paths that lose or mistranslate the error, resource lifetime (drop order,
leaked handles, use-after-free reasoning in `unsafe`), aliasing and `Send`/`Sync`
claims, race windows and lock scope, panics reachable from non-panicking APIs,
and behavior changes to callers the diff never touched. Search for the case the
author did not have in mind — empty input, single element, the maximum, the
concurrent second caller, the error branch nobody runs.

### Assertions — code

- Invariants relied on but never asserted. If a function's correctness needs
  `x < len` and nothing checks it, that is a finding.
- Assertions that do not hold, are unreachable, or are weaker than the invariant
  they claim to guard (`assert!(!v.is_empty())` where the real requirement is a
  specific length).
- `unwrap`/`expect`/`unreachable!`/`unsafe` blocks whose stated justification no
  longer follows from the surrounding code — check each one against the current
  control flow, not the comment.
- `debug_assert!` guarding something that must hold in release too.

### Assertions — claims

Every claim in the PR description, commit messages, doc comments, inline
comments, and review replies is a hypothesis to test against the code. Flag any
that the code does not support: "this is now O(n)", "safe because the caller
holds the lock", "no behavior change", "fixes #123", "the previous code leaked
here". Author confidence is not evidence. A comment that was true before the
change and is false after it is a defect, not a nit.

### LLM-isms

Machine-generated code has tells, and the tells cluster around real bugs. Flag
them on their own merits, and follow each one to see whether it is hiding a
defect:

- **Comment noise** — comments restating the line below, docstrings paraphrasing
  the signature, section banners over three lines of code.
- **Stale naming and comments** — identifiers or comments describing an earlier
  draft of the code. Often the fastest route to a real bug.
- **Defensive code for impossible states** — `None`/null checks on a value
  already narrowed, bounds checks after an indexing operation, error branches
  unreachable by construction. Either it is dead code or the invariant does not
  actually hold — determine which.
- **Invented compatibility** — deprecation shims, aliases, or migration paths for
  an API that never shipped.
- **Single-caller abstraction** — a trait, builder, config struct, or helper
  introduced with exactly one use site and no stated plan for a second.
- **Copy-paste with a missed substitution** — near-identical blocks differing by
  one identifier. Diff them character by character; the missed rename is a
  classic and is a genuine bug.
- **Reimplementation** — logic that already exists elsewhere in the workspace,
  rewritten instead of called. Search for the existing version before accepting a
  new helper.
- **Symmetry padding** — a setter added because a getter exists, enum variants or
  match arms nothing constructs, parameters no caller passes.
- **Lint silencing** — `#[allow]`, broad `catch`, or `unwrap_or_default()` added
  to quiet a diagnostic rather than address it.
- **Tautological tests** — a test that asserts the mock's return value, restates
  the implementation, or exercises the language rather than the code.
- **Prose tells** in user-facing text, changelogs, and commit messages —
  "comprehensive", "robust", "seamlessly", bullet lists that restate the diff.

### Tests

Whether the tests verify the invariants that matter, not whether coverage went
up. Name the specific untested path or unasserted invariant; "add more tests" is
not a finding.

## Step 4 — Validate before presenting (mandatory)

No suspected defect, regression, or correctness concern may appear as confirmed
until validated against the real code in the checked-out workspace:

1. Open the actual file(s) in the `workspace:` path and trace the real control
   flow / types / lifetimes — confirm the bug is reachable and its triggering
   conditions genuinely hold. Many "bugs" evaporate once you read the code around
   the hunk.
2. Where feasible, **prove it**: point to or write a failing test, construct a
   repro, or build/run the relevant code (`cargo nextest` / `cargo build` /
   `cargo clippy`, with `CARGO_TARGET_DIR` set as described above). Use
   foreground sub-agents to parallelize verification across findings.
3. Plausible-but-unconfirmed concerns are either dropped or listed **separately
   under `unverified`**, with what would be needed to confirm. Never let a hunch
   masquerade as a confirmed bug.

State *how* each confirmed finding was validated (path traced, test run, repro
built).

The gate applies to correctness and assertion findings. LLM-isms are judged by
reading — but the moment one implies a behavioral bug, that bug goes through the
gate like any other.

## Step 5 — Report

Deliver the whole report in one pass. Do **not** open by asking where to focus,
and do not scope the review down preemptively. Be direct; don't hedge. Findings
carry a `file:line` anchor from the bundle so they can be acted on directly.

1. **Preamble** — a few lines: what the change does and how it is structured.
   Enough to make the findings legible, no more. No motivation essay, no
   architecture tour unless a diagram is genuinely required to state a finding.
2. **Correctness** — confirmed defects, most severe first. Anchor, what breaks,
   the input or state that triggers it, how it was validated.
3. **Assertions** — missing or wrong invariant checks, and claims the code does
   not support. Quote the claim, then what the code does.
4. **LLM-isms** — machine-generated slop, and for each whether it is cosmetic or
   points at a defect above.
5. **Tests** — invariants that matter and go unverified.
6. **Unverified** — plausible concerns that did not pass the gate, and what would
   settle each.
7. **Unresolved in conversation** — points still contested or unanswered by the
   author. One line each. (PR only; a range has no conversation.)
8. **Review order** (mandatory) — an ordered reading plan, commit-by-commit if the
   PR is structured that way, otherwise file-by-file, with a one-line rationale
   each. Order for comprehension: the change that defines the new shape first,
   then what depends on it, mechanical churn last.

Omit any section with nothing in it, except the preamble and review order.

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
