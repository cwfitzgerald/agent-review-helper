# agent-review-helper

A Rust CLI that prepares a self-contained folder of PR review artifacts for an
LLM, using `jj` + `gh`. Invoked from inside a target repo: `agent-review-helper 8967`.

Paired with the `agent-review` skill (`~/.claude/skills/agent-review/SKILL.md`),
which runs this binary and then reviews from the bundle.

## Build / test / install

- `cargo clippy` — lint (preferred over `cargo check`).
- `cargo nextest run` — tests (pure logic only; no network/jj/gh in tests).
- `cargo install --path .` — install to `~/.cargo/bin` so the skill can call it.

## What it does (orchestration in `main.rs`)

1. Discover the jj workspace root + parse `origin` into a `RepoRef` (owner/repo/host/style).
2. `jj git fetch origin` to refresh trunk (so `jj pr-diff`'s fork point is accurate).
3. `gh pr view` for PR metadata.
4. Resolve the on-disk `Layout` and prepare folders; ensure `.agent-review/` is ignored.
5. Fork-aware fetch: add the fork as a named jj remote (named after the fork
   owner) and fetch the head branch; same-repo PRs fetch from origin.
6. `jj workspace add` at the PR head.
7. Run the artifact registry, then write `info/README.md` (the manifest).

## Module map

- `cli.rs` — clap args. `process.rs` — the only place that spawns subprocesses.
- `repo.rs` — repo discovery + `RepoRef` URL parsing (has unit tests).
- `jj.rs` / `gh.rs` — command wrappers. `gh.rs` also holds the serde models.
  **Casing gotcha:** `gh pr view --json` is camelCase; `gh api` (REST) is snake_case.
- `layout.rs` — **the only place that decides paths.** `Storage` enum
  (`in-repo` default / `cache` / `split`) so storage strategy can change without
  touching gather logic.
- `workspace.rs` — ignore-file handling + folder prep / `--force` teardown.
- `context.rs` — `ReviewContext` handed to every artifact.
- `artifacts/` — `Artifact` trait + registry. **Add a new gathered file here**
  (e.g. CI logs, related issues) by implementing `Artifact` and adding it to
  `default_registry()`; nothing else changes.
- `manifest.rs` — writes the `README.md` orienting map in the info folder.

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
