use std::fmt::Write as _;

use chrono::{DateTime, Utc};
use cm_core::Entry;

use super::super::{
    SNIPPET_MAX_BYTES, collapse_whitespace, compute_dedup_hints, relative_age, smart_snippet,
};
use super::layout::Layout;

pub(super) fn render_entries(out: &mut String, layout: &Layout) {
    out.push_str("entries:\n");
    if layout.rows.is_empty() {
        out.push_str("  []\n");
        return;
    }

    // Continuation lines align with the start of the title column on
    // line 1:
    //   "  - Title"              -> 4 (list indent + "- ")
    //   "  - X.XX  Title"        -> 4 + 4 (score) + 2 (gap)
    let cont_indent = if layout.show_score {
        " ".repeat(4 + 4 + 2)
    } else {
        " ".repeat(4)
    };

    // Intra-response dedup: first occurrence of each content-hash
    // prefix is the leader; every later row whose prefix collides
    // with a leader gets a `dup_of: <short leader id>` annotation in
    // its trailing comment. Computed once per render pass.
    let entries: Vec<&Entry> = layout.rows.iter().map(|r| &r.entry).collect();
    let dedup = compute_dedup_hints(&entries);

    for (i, row) in layout.rows.iter().enumerate() {
        if layout.show_score {
            let s = layout.norm_scores.get(i).copied().unwrap_or(0.0);
            let _ = writeln!(out, "  - {s:.2}  {}", row.entry.title);
        } else {
            let _ = writeln!(out, "  - {}", row.entry.title);
        }

        let snippet = smart_snippet(
            &row.entry.body,
            layout.query,
            layout.highlight_style,
            SNIPPET_MAX_BYTES,
        );
        let snippet_line = collapse_whitespace(&snippet);
        if !snippet_line.is_empty() {
            let _ = writeln!(out, "{cont_indent}{snippet_line}");
        }

        let dup_of = dedup
            .get(&row.entry.id)
            .map(|leader_uuid| leader_uuid.to_string());
        let rels = layout
            .relation_counts
            .get(&row.entry.id)
            .copied()
            .unwrap_or(0);
        let comment = render_row_comment(&row.entry, layout.now, dup_of.as_deref(), rels);
        let _ = writeln!(out, "{cont_indent}# {comment}");
    }
}

fn render_row_comment(
    entry: &Entry,
    now: DateTime<Utc>,
    dup_of: Option<&str>,
    rels: u32,
) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(6);
    parts.push(format!("scope: {}", entry.scope_path));
    parts.push(format!("kind: {}", entry.kind.as_str()));
    let tags: &[String] = entry
        .meta
        .as_ref()
        .map(|m| m.tags.as_slice())
        .unwrap_or(&[]);
    if !tags.is_empty() {
        parts.push(format!("tags: {}", tags.join(", ")));
    }
    parts.push(format!("age: {}", relative_age(entry.updated_at, now)));
    if let Some(dup) = dup_of {
        parts.push(format!("dup_of: {dup}"));
    }
    if rels > 0 {
        parts.push(format!("rels: {rels}"));
    }
    parts.join("  ")
}
