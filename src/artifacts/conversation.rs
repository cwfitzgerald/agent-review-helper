//! The chronological conversation artifact: PR body + metadata, general
//! comments, review submissions, and inline (code-anchored) comments, all
//! merged into one timeline so the reviewer sees the discussion in order.

use anyhow::Result;
use std::fmt::Write as _;
use std::fs;

use super::{Artifact, Produced};
use crate::context::ReviewContext;
use crate::gh;

pub struct ConversationArtifact;

impl Artifact for ConversationArtifact {
    fn id(&self) -> &'static str {
        "conversation"
    }

    fn produce(&self, ctx: &ReviewContext) -> Result<Produced> {
        let slug = ctx.slug();
        let issue_comments = gh::issue_comments(&slug, ctx.pr)?;
        let reviews = gh::reviews(&slug, ctx.pr)?;
        let review_comments = gh::review_comments(&slug, ctx.pr)?;

        let mut events: Vec<Event> = Vec::new();

        for c in &issue_comments {
            events.push(Event {
                time: c.created_at.clone(),
                author: c.user.login.clone(),
                url: c.html_url.clone(),
                kind: Kind::Comment,
                body: c.body.clone(),
            });
        }
        for r in &reviews {
            // Pending reviews have no submitted_at; skip them.
            let Some(time) = r.submitted_at.clone() else {
                continue;
            };
            events.push(Event {
                time,
                author: r.user.login.clone(),
                url: r.html_url.clone(),
                kind: Kind::Review { state: r.state.clone() },
                body: r.body.clone(),
            });
        }
        for c in &review_comments {
            let line = c.line.or(c.original_line);
            events.push(Event {
                time: c.created_at.clone(),
                author: c.user.login.clone(),
                url: c.html_url.clone(),
                kind: Kind::Inline {
                    path: c.path.clone(),
                    line,
                    diff_hunk: c.diff_hunk.clone(),
                    is_reply: c.in_reply_to_id.is_some(),
                },
                body: c.body.clone(),
            });
        }

        // ISO-8601 UTC timestamps sort lexically.
        events.sort_by(|a, b| a.time.cmp(&b.time));

        let markdown = render(ctx, &events);
        let file = ctx.layout.info.join("conversation.md");
        fs::write(&file, markdown)?;
        Ok(Produced {
            file,
            description: "Chronological PR conversation: description, comments, reviews, and \
                inline threads."
                .into(),
        })
    }
}

enum Kind {
    Comment,
    Review { state: String },
    Inline {
        path: String,
        line: Option<u64>,
        diff_hunk: String,
        is_reply: bool,
    },
}

struct Event {
    time: String,
    author: String,
    url: String,
    kind: Kind,
    body: String,
}

fn render(ctx: &ReviewContext, events: &[Event]) -> String {
    let pr = &ctx.pr_info;
    let mut out = String::new();

    writeln!(out, "# PR #{} — {}", pr.number, pr.title).ok();
    writeln!(out).ok();
    writeln!(out, "- **Repo**: {}", ctx.slug()).ok();
    writeln!(out, "- **Author**: @{}", pr.author.login).ok();
    writeln!(
        out,
        "- **State**: {}{}",
        pr.state,
        if pr.is_draft { " (draft)" } else { "" }
    )
    .ok();
    writeln!(out, "- **Base ← Head**: `{}` ← `{}`", pr.base_ref_name, pr.head_ref_name).ok();
    if !pr.mergeable.is_empty() {
        writeln!(out, "- **Mergeable**: {}", pr.mergeable).ok();
    }
    writeln!(
        out,
        "- **Changes**: +{} / -{} across {} file(s)",
        pr.additions, pr.deletions, pr.changed_files
    )
    .ok();
    if !pr.labels.is_empty() {
        let names: Vec<&str> = pr.labels.iter().map(|l| l.name.as_str()).collect();
        writeln!(out, "- **Labels**: {}", names.join(", ")).ok();
    }
    writeln!(out, "- **URL**: {}", pr.url).ok();
    writeln!(out, "- **Opened**: {}", pr.created_at).ok();
    writeln!(out).ok();

    writeln!(out, "## Description").ok();
    writeln!(out).ok();
    writeln!(out, "{}", body_or_placeholder(&pr.body)).ok();
    writeln!(out).ok();

    writeln!(out, "## Timeline").ok();
    writeln!(out).ok();
    if events.is_empty() {
        writeln!(out, "_No comments or reviews yet._").ok();
        return out;
    }

    for ev in events {
        match &ev.kind {
            Kind::Comment => {
                writeln!(out, "### 💬 @{} — comment · {}", ev.author, ev.time).ok();
            }
            Kind::Review { state } => {
                writeln!(
                    out,
                    "### {} @{} — review ({}) · {}",
                    review_marker(state),
                    ev.author,
                    state,
                    ev.time
                )
                .ok();
            }
            Kind::Inline { path, line, diff_hunk, is_reply } => {
                let loc = match line {
                    Some(l) => format!("{path}:{l}"),
                    None => path.clone(),
                };
                let reply = if *is_reply { " (reply)" } else { "" };
                writeln!(out, "### 📝 @{} — inline on `{}`{} · {}", ev.author, loc, reply, ev.time).ok();
                if !diff_hunk.is_empty() {
                    writeln!(out).ok();
                    writeln!(out, "```diff").ok();
                    writeln!(out, "{}", diff_hunk).ok();
                    writeln!(out, "```").ok();
                }
            }
        }
        writeln!(out).ok();
        let body = body_or_placeholder(&ev.body);
        writeln!(out, "{}", body).ok();
        if !ev.url.is_empty() {
            writeln!(out).ok();
            writeln!(out, "<{}>", ev.url).ok();
        }
        writeln!(out).ok();
        writeln!(out, "---").ok();
        writeln!(out).ok();
    }

    out
}

fn review_marker(state: &str) -> &'static str {
    match state {
        "APPROVED" => "✅",
        "CHANGES_REQUESTED" => "❌",
        "DISMISSED" => "🚫",
        _ => "🗒️",
    }
}

fn body_or_placeholder(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        "_(no text)_".to_string()
    } else {
        trimmed.to_string()
    }
}
