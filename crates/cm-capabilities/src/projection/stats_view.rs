//! `cx_stats` YAML-text formatter for MCP response bodies.
//!
//! Consumed by `cx_stats` (via the wire-swap sub that lands the YAML
//! envelope) to replace the ad-hoc `serde_json!` blob built in
//! `crates/cm-cli/src/mcp/tools/stats.rs` with a compact,
//! agent-legible YAML view. The target shape is described in
//! `research/cx-response-payload-redesign-context-matters.md` §5.2.4.
//!
//! The main deliverable over the old shape is the `scope_tree:` block:
//! the current handler emits a flat list of `{path, label, entry_count}`
//! records under a `scope_tree` key, which is a tree in name only.
//! This module turns the flat list into a real indented pre-order walk
//! keyed by the `/` separators in each `scope_path`, so the reader can
//! see the project/repo/session hierarchy at a glance. Scopes whose
//! parent path is missing from the result set render at their natural
//! depth anyway, and a trailing `# N orphaned scopes - parent not in
//! list` advisory surfaces the count.
//!
//! The formatter is pure text: no I/O, no allocations beyond the
//! output string and its temporaries. Unlike the recall/browse/get
//! formatters it takes no reference `now`, because every field it
//! renders is purely a function of the input struct (no relative-age
//! columns).

use std::collections::HashSet;
use std::fmt::Write as _;

use cm_core::StoreStats;

use super::fmt_with_commas;
use crate::stats::{ScopeTreeNode, StatsResult};

/// Fixed column at which scope-tree counts are right-aligned. Wide
/// enough to leave at least two spaces of padding after typical scope
/// labels (`project:helioy`, `repo:context-matters`) without pushing
/// deeper nodes into awkward wrap territory on a standard terminal.
/// Long labels fall back to the minimum padding rule in
/// [`render_scope_tree_line`].
const SCOPE_TREE_COUNT_COL: usize = 42;

/// Minimum spaces between a scope-tree label and its count when the
/// label overflows [`SCOPE_TREE_COUNT_COL`]. Preserves visual separation
/// even on pathological inputs; the structured `scope_tree` field still
/// carries the exact value, so rendering can prioritise legibility.
const SCOPE_TREE_MIN_GAP: usize = 2;

/// Maximum number of tags shown in the `top_tags:` block. The full list
/// is already capped by `tag_sort` elsewhere in the capability layer;
/// this is a safety trim for pathological stores with hundreds of tags.
const TOP_TAGS_LIMIT: usize = 10;

/// Render a [`StatsResult`] as YAML-annotated text for the `cx_stats`
/// MCP response body. See the module docstring for the target shape.
///
/// The formatter takes no reference `now`: every rendered field is a
/// pure function of the input struct, so snapshot tests do not need to
/// pin a clock. Matches the deterministic-inputs convention shared with
/// [`format_get_view`](super::format_get_view) for fixtures that carry
/// no relative-age columns.
pub fn format_stats_view(result: &StatsResult) -> String {
    let mut out = String::with_capacity(1024);
    out.push_str("---\n");
    render_counters(&mut out, &result.stats);
    render_kinds(&mut out, &result.stats);
    render_top_tags(&mut out, &result.stats);
    render_scope_tree(&mut out, &result.scope_tree);
    out
}

/// Render the top-level counter header (`active`, `superseded`,
/// `scopes`, `relations`, `db_size`). Every value is pre-formatted with
/// comma thousands separators so 5-6 digit counts read at a glance; the
/// `db_size` line uses [`format_bytes`] for human-readable units.
fn render_counters(out: &mut String, stats: &StoreStats) {
    let _ = writeln!(out, "active: {}", fmt_with_commas(stats.active_entries));
    let _ = writeln!(
        out,
        "superseded: {}",
        fmt_with_commas(stats.superseded_entries)
    );
    let _ = writeln!(out, "scopes: {}", fmt_with_commas(stats.scopes));
    let _ = writeln!(out, "relations: {}", fmt_with_commas(stats.relations));
    let _ = writeln!(out, "db_size: {}", format_bytes(stats.db_size_bytes));
}

/// Render the `kinds:` block. Sorted descending by count with
/// alphabetical tiebreak, column-aligned so the count column lines up
/// regardless of the longest kind name in the set. Empty sets render as
/// a blank `kinds:` header followed by the next section, not as a
/// suppressed block — callers querying an empty store should still see
/// the field exist.
fn render_kinds(out: &mut String, stats: &StoreStats) {
    out.push('\n');
    out.push_str("kinds:\n");
    if stats.entries_by_kind.is_empty() {
        return;
    }
    let mut rows: Vec<(&String, &u64)> = stats.entries_by_kind.iter().collect();
    rows.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    let width = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (k, v) in rows {
        let _ = writeln!(out, "  {k:<width$}  {count}", count = fmt_with_commas(*v));
    }
}

/// Render the `top_tags:` block. Caps at [`TOP_TAGS_LIMIT`] rows. The
/// capability layer already sorts `entries_by_tag` by the caller's
/// `tag_sort` request, so this helper preserves that order and does not
/// re-sort: a caller that asked for alphabetical tags gets alphabetical
/// tags here too. Empty sets still emit the `top_tags:` header for the
/// same reason as [`render_kinds`].
fn render_top_tags(out: &mut String, stats: &StoreStats) {
    out.push('\n');
    out.push_str("top_tags:\n");
    if stats.entries_by_tag.is_empty() {
        return;
    }
    let top = &stats.entries_by_tag[..stats.entries_by_tag.len().min(TOP_TAGS_LIMIT)];
    let width = top.iter().map(|tc| tc.tag.len()).max().unwrap_or(0);
    for tc in top {
        let _ = writeln!(
            out,
            "  {tag:<width$}  {count}",
            tag = tc.tag,
            count = fmt_with_commas(tc.count)
        );
    }
}

/// Render the `scope_tree:` block as a real indented tree. Sorts the
/// flat list by path (lex ascending gives the pre-order walk), computes
/// the depth as `path.split('/').count() - 1`, and indents by two spaces
/// per level. Counts are right-aligned at [`SCOPE_TREE_COUNT_COL`].
///
/// Orphans (scopes whose `/`-parent path is not present in the input
/// list) still render at their natural depth, and a trailing `# N
/// orphaned scopes - parent not in list` advisory surfaces the count so
/// the caller can tell the tree is incomplete without diffing against
/// the structured data. The root scope `global` has no parent and is
/// never an orphan.
fn render_scope_tree(out: &mut String, tree: &[ScopeTreeNode]) {
    out.push('\n');
    out.push_str("scope_tree:\n");
    if tree.is_empty() {
        return;
    }
    let mut sorted: Vec<&ScopeTreeNode> = tree.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));

    let paths: HashSet<&str> = sorted.iter().map(|n| n.path.as_str()).collect();
    let mut orphan_count = 0usize;

    for node in &sorted {
        let depth = scope_depth(&node.path);
        render_scope_tree_line(out, depth, &node.label, node.entry_count);
        if is_orphan(&node.path, &paths) {
            orphan_count += 1;
        }
    }

    if orphan_count > 0 {
        let _ = writeln!(
            out,
            "\n# {orphan_count} orphaned scopes - parent not in list"
        );
    }
}

/// Emit a single `scope_tree` row as depth indent, label, padding,
/// count. The label/padding layout is the only non-trivial piece. When
/// the row fits within [`SCOPE_TREE_COUNT_COL`], the gap is sized so
/// the right edge of the count lands exactly on that column. When the
/// label overflows, the count falls back to [`SCOPE_TREE_MIN_GAP`]
/// spaces of separation so the row still reads.
fn render_scope_tree_line(out: &mut String, depth: usize, label: &str, count: u64) {
    let indent = "  ".repeat(depth + 1);
    let count_str = fmt_with_commas(count);
    let line_len = indent.len() + label.len() + count_str.len();
    let pad = SCOPE_TREE_COUNT_COL
        .saturating_sub(line_len)
        .max(SCOPE_TREE_MIN_GAP);
    let _ = writeln!(
        out,
        "{indent}{label}{spaces}{count_str}",
        spaces = " ".repeat(pad)
    );
}

/// Compute tree depth from a scope path. `global` is depth 0,
/// `global/project:x` is depth 1, `global/project:x/repo:y` is depth 2,
/// and so on. Returns 0 for an empty string (defensive: no pub API
/// should feed this helper an empty path).
fn scope_depth(path: &str) -> usize {
    if path.is_empty() {
        return 0;
    }
    path.split('/').count() - 1
}

/// Whether `path`'s parent is absent from the known-paths set. Returns
/// `false` for root (`global`) and any path that cannot be split, since
/// they have no parent to check.
fn is_orphan(path: &str, paths: &HashSet<&str>) -> bool {
    let Some(idx) = path.rfind('/') else {
        return false;
    };
    let parent = &path[..idx];
    !paths.contains(parent)
}

/// Human-readable byte count: `B`, `KB`, `MB`, `GB`, `TB`.
///
/// Uses base-1024 thresholds (the SI-lite convention shared by `ls -h`
/// and most developer tooling), with one decimal place above 1 KB and
/// an integer `B` column below. Never renders trailing zeros (`4.0 MB`
/// becomes `4 MB`), so the `db_size:` line stays tight.
///
/// Private to this module for now. If another view needs byte
/// formatting it should be promoted to `aggregation.rs` alongside
/// `fmt_with_commas`; until then, keeping it local avoids a one-caller
/// export.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes < KB {
        return format!("{bytes} B");
    }
    let (value, unit) = if bytes < MB {
        (bytes as f64 / KB as f64, "KB")
    } else if bytes < GB {
        (bytes as f64 / MB as f64, "MB")
    } else if bytes < TB {
        (bytes as f64 / GB as f64, "GB")
    } else {
        (bytes as f64 / TB as f64, "TB")
    };
    // Strip `.0` tail: `4.0 MB` -> `4 MB`, `4.2 MB` stays `4.2 MB`.
    let rendered = format!("{value:.1}");
    let trimmed = rendered.strip_suffix(".0").unwrap_or(&rendered);
    format!("{trimmed} {unit}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cm_core::TagCount;
    use std::collections::HashMap;

    fn empty_stats() -> StoreStats {
        StoreStats {
            active_entries: 0,
            superseded_entries: 0,
            scopes: 0,
            relations: 0,
            entries_by_kind: HashMap::new(),
            entries_by_scope: HashMap::new(),
            entries_by_tag: Vec::new(),
            db_size_bytes: 0,
        }
    }

    fn node(path: &str, label: &str, entry_count: u64) -> ScopeTreeNode {
        ScopeTreeNode {
            path: path.to_owned(),
            kind: "workspace".to_owned(),
            label: label.to_owned(),
            entry_count,
        }
    }

    #[test]
    fn format_bytes_handles_b_kb_mb_gb_tb_boundaries() {
        // Below 1 KB: bytes literal, no decimals.
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1), "1 B");
        assert_eq!(format_bytes(1023), "1023 B");
        // Exactly 1 KB crosses into the KB branch.
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024 - 1), "1024 KB");
        // MB boundary.
        assert_eq!(format_bytes(1024 * 1024), "1 MB");
        assert_eq!(format_bytes(4_404_019), "4.2 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 - 1), "1024 MB");
        // GB boundary.
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1 GB");
        assert_eq!(
            format_bytes((1.5 * 1024.0 * 1024.0 * 1024.0) as u64),
            "1.5 GB"
        );
        // TB boundary.
        assert_eq!(format_bytes(1024_u64.pow(4)), "1 TB");
        assert_eq!(format_bytes(2 * 1024_u64.pow(4)), "2 TB");
    }

    #[test]
    fn scope_depth_counts_slash_segments() {
        assert_eq!(scope_depth("global"), 0);
        assert_eq!(scope_depth("global/project:helioy"), 1);
        assert_eq!(scope_depth("global/project:helioy/repo:cm"), 2);
        assert_eq!(scope_depth("global/project:helioy/repo:cm/session:abc"), 3);
        assert_eq!(
            scope_depth("global/project:helioy/repo:cm/session:abc/thread:1"),
            4
        );
        // Empty string defensively returns 0.
        assert_eq!(scope_depth(""), 0);
    }

    #[test]
    fn is_orphan_detects_missing_parent() {
        let paths: HashSet<&str> = ["global", "global/project:helioy/repo:cm"]
            .into_iter()
            .collect();
        // Root is never an orphan.
        assert!(!is_orphan("global", &paths));
        // Parent `global/project:helioy` is absent.
        assert!(is_orphan("global/project:helioy/repo:cm", &paths));
        // Parent `global` is present.
        assert!(!is_orphan("global/project:helioy", &{
            let mut p: HashSet<&str> = HashSet::new();
            p.insert("global");
            p
        }));
    }

    #[test]
    fn render_scope_tree_line_right_aligns_at_fixed_column() {
        // Short label: padding should bring the right edge of the count
        // to SCOPE_TREE_COUNT_COL.
        let mut out = String::new();
        render_scope_tree_line(&mut out, 0, "global", 1042);
        // "  global" = 8 chars, "1,042" = 5 chars, pad = 42 - 8 - 5 = 29.
        assert_eq!(out, format!("  global{}1,042\n", " ".repeat(29)));
    }

    #[test]
    fn render_scope_tree_line_overflow_falls_back_to_min_gap() {
        // Long label that overflows col 42: the helper should fall back
        // to the minimum 2-space gap instead of returning a panic or
        // a zero-width padding.
        let mut out = String::new();
        render_scope_tree_line(&mut out, 3, "very:long:scope:label:that:overflows", 42);
        // indent "        " (8) + label (36) + gap (2) + "42" (2) = 48.
        assert!(
            out.ends_with("  42\n"),
            "expected a min-gap fallback, got: {out:?}",
        );
    }

    #[test]
    fn render_scope_tree_indents_by_depth_zero_through_four() {
        // A synthetic tree spanning depth 0..4. Lex-sort produces a
        // correct pre-order walk because each path is a prefix of its
        // child.
        let tree = vec![
            node("global", "global", 1000),
            node("global/project:helioy", "project:helioy", 200),
            node("global/project:helioy/repo:cm", "repo:cm", 50),
            node(
                "global/project:helioy/repo:cm/session:abc",
                "session:abc",
                10,
            ),
            node(
                "global/project:helioy/repo:cm/session:abc/thread:1",
                "thread:1",
                3,
            ),
        ];
        let mut out = String::new();
        render_scope_tree(&mut out, &tree);
        // Each depth should prefix its label with depth+1 pairs of spaces.
        assert!(out.contains("\n  global"));
        assert!(out.contains("\n    project:helioy"));
        assert!(out.contains("\n      repo:cm"));
        assert!(out.contains("\n        session:abc"));
        assert!(out.contains("\n          thread:1"));
        // No orphans in this fixture.
        assert!(!out.contains("orphaned scopes"));
    }

    #[test]
    fn render_scope_tree_emits_orphan_advisory_when_parent_missing() {
        // `global/project:orphaned/repo:x` has no corresponding
        // `global/project:orphaned` in the tree, so it is an orphan.
        let tree = vec![
            node("global", "global", 1),
            node("global/project:orphaned/repo:x", "repo:x", 5),
        ];
        let mut out = String::new();
        render_scope_tree(&mut out, &tree);
        assert!(out.contains("repo:x"));
        assert!(
            out.contains("# 1 orphaned scopes - parent not in list"),
            "expected orphan advisory, got: {out:?}",
        );
    }

    #[test]
    fn render_kinds_column_aligns_to_max_width() {
        let mut stats = empty_stats();
        stats.entries_by_kind.insert("observation".to_owned(), 748);
        stats.entries_by_kind.insert("fact".to_owned(), 201);
        stats.entries_by_kind.insert("decision".to_owned(), 87);
        let mut out = String::new();
        render_kinds(&mut out, &stats);
        // The widest kind is "observation" (11 chars). Every row should
        // right-pad the kind name to 11 chars before the count column.
        assert!(out.contains("  observation  748"));
        assert!(out.contains("  fact         201"));
        assert!(out.contains("  decision     87"));
    }

    #[test]
    fn render_kinds_sorted_descending_by_count() {
        let mut stats = empty_stats();
        stats.entries_by_kind.insert("a".to_owned(), 10);
        stats.entries_by_kind.insert("b".to_owned(), 100);
        stats.entries_by_kind.insert("c".to_owned(), 50);
        let mut out = String::new();
        render_kinds(&mut out, &stats);
        let b_idx = out.find("b  100").unwrap();
        let c_idx = out.find("c  50").unwrap();
        let a_idx = out.find("a  10").unwrap();
        assert!(
            b_idx < c_idx && c_idx < a_idx,
            "expected descending count order, got: {out}",
        );
    }

    #[test]
    fn render_top_tags_caps_at_top_tags_limit() {
        let mut stats = empty_stats();
        // 12 tags: the 11th and 12th must not be rendered.
        for i in 0..12 {
            stats.entries_by_tag.push(TagCount {
                tag: format!("tag{i:02}"),
                count: 100 - i as u64,
            });
        }
        let mut out = String::new();
        render_top_tags(&mut out, &stats);
        for i in 0..TOP_TAGS_LIMIT {
            assert!(out.contains(&format!("tag{i:02}")));
        }
        assert!(!out.contains("tag10"));
        assert!(!out.contains("tag11"));
    }

    #[test]
    fn render_counters_uses_thousand_separators() {
        let mut stats = empty_stats();
        stats.active_entries = 1_342;
        stats.superseded_entries = 89;
        stats.scopes = 17;
        stats.relations = 1_234_567;
        stats.db_size_bytes = 4_404_019;
        let mut out = String::new();
        render_counters(&mut out, &stats);
        assert!(out.contains("active: 1,342"));
        assert!(out.contains("superseded: 89"));
        assert!(out.contains("scopes: 17"));
        assert!(out.contains("relations: 1,234,567"));
        assert!(out.contains("db_size: 4.2 MB"));
    }

    #[test]
    fn format_stats_view_emits_all_sections() {
        let mut stats = empty_stats();
        stats.active_entries = 5;
        stats.entries_by_kind.insert("fact".to_owned(), 5);
        stats.entries_by_tag.push(TagCount {
            tag: "rust".to_owned(),
            count: 3,
        });
        let result = StatsResult {
            stats,
            scope_tree: vec![node("global", "global", 5)],
        };
        let rendered = format_stats_view(&result);
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("active: 5"));
        assert!(rendered.contains("kinds:"));
        assert!(rendered.contains("top_tags:"));
        assert!(rendered.contains("scope_tree:"));
    }
}
