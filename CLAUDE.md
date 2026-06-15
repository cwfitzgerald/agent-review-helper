# agent-review-helper

A Rust CLI that prepares a self-contained folder of review artifacts for an LLM,
using `jj` (+ `gh` for PRs). Invoked from inside a target repo. Two modes:

- `agent-review-helper 8967` (or `pr 8967`) — GitHub PR review.
- `agent-review-helper range -f <from> -t <to>` — a local jj revision range.

A bare PR number is a backwards-compatible shorthand for `pr <PR>`.

Paired with the `agent-review` skill (`~/.claude/skills/agent-review/SKILL.md`),
which runs this binary and then reviews from the bundle.

## Build / test / install

- `cargo clippy` — lint (preferred over `cargo check`).
- `cargo nextest run` — tests (pure logic only; no network/jj/gh in tests).
- `cargo install --path .` — install to `~/.cargo/bin` so the skill can call it.

## What it does (orchestration in `main.rs`)

`run()` resolves the subcommand (bare number → `pr`) and dispatches to `run_pr`
or `run_range`. Both share `layout` (paths), `workspace` (folder prep / teardown
/ `--clean`), and the `manifest` writer; only the gather differs.

`run_pr` (PR mode):

1. Discover the jj workspace root + pick the base remote (`--remote`, else
   `origin`, else `upstream`, else the sole remote) and parse its URL into a
   `RepoRef` (owner/repo/host/style). Works in repos with no `origin` (e.g. a
   wgpu checkout where `upstream` = gfx-rs and the fork is a named remote).
2. `--clean` short-circuits here: tear down this PR's workspace + bundle and exit
   (no fetch, no `gh`). Idempotent.
3. `jj git fetch <base remote>` to refresh trunk (so `jj pr-diff`'s fork point is accurate).
4. `gh pr view` for PR metadata.
5. Prepare folders; ensure `.agent-review/` is ignored.
6. Fork-aware fetch: add the fork as a named jj remote (named after the fork
   owner) and fetch the head branch; same-repo PRs fetch from the base remote.
7. `jj workspace add` at the PR head.
8. Run the artifact registry, then write `info/README.md` (the manifest).

`run_range` (range mode): no remote/`gh`/fetch. Resolve `from`/`to` to short
commit ids for a stable `range-<from>-<to>` bundle id (so `--clean` finds it
again), prepare folders, `jj workspace add` at `to`, then write `range.diff`
(`jj diff -f from -t to`) + `commits.txt` (`jj log -r from..to`) and the manifest.

## Module map

- `cli.rs` — clap args: `Pr`/`Range` subcommands + a bare-number shorthand
  (`Cli::resolve`). Has parse unit tests. `process.rs` — the only place that
  spawns subprocesses.
- `repo.rs` — repo discovery, base-remote selection (`pick_base_remote`), and
  `RepoRef` URL parsing (has unit tests). PR mode only.
- `jj.rs` / `gh.rs` — command wrappers. `gh.rs` also holds the serde models.
  **Casing gotcha:** `gh pr view --json` is camelCase; `gh api` (REST) is snake_case.
- `layout.rs` — **the only place that decides paths.** Mode-agnostic:
  `resolve(strategy, repo_root, namespace, bundle)`. `Storage` enum (`in-repo`
  default / `cache` / `split`) so storage strategy can change without touching
  gather logic.
- `workspace.rs` — ignore-file handling + folder prep / `--force` teardown +
  `--clean` (public `clean()` wraps the same teardown, reports no-op). Keyed on a
  workspace name string (`workspace_name(bundle)` → `arh-<bundle>`), not a PR.
- `context.rs` — `ReviewContext` handed to every PR artifact.
- `artifacts/` — PR `Artifact` trait + registry. **Add a new gathered PR file
  here** (e.g. CI logs, related issues) by implementing `Artifact` and adding it
  to `default_registry()`; nothing else changes.
- `range.rs` — range-mode gather (`range.diff` + `commits.txt`). Simple direct
  functions, not the `Artifact` registry (fixed, small artifact set).
- `manifest.rs` — writes the `README.md` orienting map; `write` (PR) and
  `write_range` share the artifacts list + `CARGO_TARGET_DIR` building section.

## Conventions

- LF line endings. Errors via `anyhow` with `.context()` at call sites.
- All `gh` calls pass `--repo owner/repo` explicitly — never rely on `gh`
  auto-detecting the repo from a (possibly non-colocated) jj working dir.
- Read-only `gh`; the tool never commits. `jj` mutations are limited to
  fetch / remote add / workspace add (+ workspace forget under `--force`).

## Known risk to validate

The default `in-repo` storage nests the PR workspace inside the parent repo
tree. This relies on the `.agent-review/` exclude entry being effective so the
parent workspace never snapshots the nested checkout. If jj churns on it, switch
to `--storage split` (workspace in cache, info in repo) or `--storage cache`.
Not yet validated end-to-end against a real wgpu PR.
