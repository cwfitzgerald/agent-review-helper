# agent-review-helper

A small Rust CLI that prepares a self-contained folder of review artifacts for an
LLM, so I can review changes locally without feeding the model a pile of unneeded
context. Run it from inside a target repo.

**Review a GitHub PR:**

```
agent-review-helper 8967
```

It creates a `jj` workspace at the PR head and gathers, into
`.agent-review/pr-<n>/info/`:

- `pr-diff.diff` — the local merge-base diff (`jj pr-diff`).
- `upstream.diff` — GitHub's view of the diff (`gh pr diff`), for cross-checking.
- `conversation.md` — the PR description, comments, reviews, and inline threads,
  merged into one chronological timeline.
- `README.md` — a manifest tying it together, plus the workspace path for building
  / navigating the full checkout.

**Review a local revision range** (no GitHub, no network):

```
agent-review-helper range -f <from> -t <to>     # -t defaults to @
```

This checks out `<to>` in a workspace and gathers `range.diff`
(`jj diff -f <from> -t <to>`) + `commits.txt` (the range's commit log) into
`.agent-review/range-<from>-<to>/info/`.

When you're done, tear down the workspace and bundle (works for either mode):

```
agent-review-helper 8967 --clean
agent-review-helper range -f <from> -t <to> --clean
```

It pairs with a Claude Code skill (`agent-review`) that runs the binary and reviews
from the bundle.

## Install

```
cargo install --git https://github.com/cwfitzgerald/agent-review-helper --locked
```

Add `--force` to update an existing install to the latest.

## Requirements

- [`jj`](https://github.com/jj-vcs/jj) — the target must be a jj repo with a
  remote pointing at the PR's GitHub repo. The base remote is auto-detected
  (`origin`, else `upstream`, else the sole remote); override with `--remote`.
- [`gh`](https://cli.github.com/) — authenticated, for PR metadata and diffs.

## Status

This is **vibe-coded for my own purposes** — built quickly to fit my personal
review workflow (wgpu and other jj-managed Rust repos). No stability guarantees,
no support, and the design will change as I extend it. Use at your own risk.

## License

Licensed under any of

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- zlib License ([LICENSE-ZLIB](LICENSE-ZLIB))

at your option.
