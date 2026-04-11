//! Snapshot tests for `cm_capabilities::projection::format_stats_view`.
//!
//! The `stats_view` formatter is exercised by a single realistic full
//! fixture that hits every section of the rendered envelope: top-level
//! counters with thousands separators, human-readable `db_size`, a
//! kinds block with column-aligned rows, a top-tags block in store
//! order, and a real indented `scope_tree` spanning depth 0..2 with no
//! orphans. The supporting unit tests in `projection::stats_view::tests`
//! already cover depth 0..4, orphan detection, `format_bytes` across
//! byte-size boundaries, and column alignment on synthetic fixtures;
//! this integration test guards the end-to-end wire shape by diffing
//! byte-for-byte against a golden on disk.
//!
//! If the wire shape ever needs to change intentionally, update the
//! golden file at `tests/snapshots/stats_view.txt` alongside the change.

use std::collections::HashMap;

use cm_capabilities::projection::format_stats_view;
use cm_capabilities::stats::{ScopeTreeNode, StatsResult};
use cm_core::{StoreStats, TagCount};

const GOLDEN_STATS_VIEW: &str = include_str!("snapshots/stats_view.txt");

/// Realistic full stats fixture matching the §5.2.4 target shape in
/// `research/cx-response-payload-redesign-context-matters.md`. The
/// numbers are deliberately chosen to exercise thousands separators,
/// column alignment across a range of label widths, and a 4.2 MB
/// `db_size` that renders with one decimal place.
fn realistic_stats_fixture() -> StatsResult {
    let mut entries_by_kind: HashMap<String, u64> = HashMap::new();
    entries_by_kind.insert("observation".to_owned(), 748);
    entries_by_kind.insert("fact".to_owned(), 201);
    entries_by_kind.insert("decision".to_owned(), 87);
    entries_by_kind.insert("lesson".to_owned(), 81);
    entries_by_kind.insert("preference".to_owned(), 34);
    entries_by_kind.insert("feedback".to_owned(), 12);

    // entries_by_tag: store default is count DESC, preserved by the
    // capability layer when TagSort::Count is requested. The fixture
    // holds the pre-sorted order the formatter receives.
    let entries_by_tag = vec![
        TagCount {
            tag: "session-log".to_owned(),
            count: 113,
        },
        TagCount {
            tag: "helioy".to_owned(),
            count: 98,
        },
        TagCount {
            tag: "cm".to_owned(),
            count: 41,
        },
        TagCount {
            tag: "projection".to_owned(),
            count: 28,
        },
    ];

    let stats = StoreStats {
        active_entries: 1_342,
        superseded_entries: 89,
        scopes: 17,
        relations: 201,
        entries_by_kind,
        entries_by_scope: HashMap::new(),
        entries_by_tag,
        db_size_bytes: 4_404_019,
    };

    let scope_tree = vec![
        node("global", "global", 1_042),
        node("global/project:helioy", "project:helioy", 203),
        node(
            "global/project:helioy/repo:context-matters",
            "repo:context-matters",
            78,
        ),
        node("global/project:helioy/repo:fmm", "repo:fmm", 45),
        node("global/project:nancyr", "project:nancyr", 97),
    ];

    StatsResult { stats, scope_tree }
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
fn format_stats_view_realistic_fixture_matches_golden() {
    let result = realistic_stats_fixture();
    let rendered = format_stats_view(&result);
    assert_eq!(
        rendered, GOLDEN_STATS_VIEW,
        "stats_view rendering does not match golden\n--- rendered ---\n{rendered}\n--- end ---",
    );
}

/// Guard: the `db_size:` line must render the MB value with one
/// decimal place (`4.2 MB`), not `4194304 B` or `4.0 MB`. Complements
/// the unit test for `format_bytes` by asserting the integration
/// pipeline actually reaches it.
#[test]
fn format_stats_view_renders_db_size_in_mb() {
    let result = realistic_stats_fixture();
    let rendered = format_stats_view(&result);
    assert!(
        rendered.contains("db_size: 4.2 MB"),
        "expected db_size: 4.2 MB, got:\n{rendered}",
    );
}

/// Guard: every top-level counter must carry thousand separators on
/// four-digit+ values, to avoid regressing the `fmt_with_commas`
/// promotion from `recall_view.rs` into `aggregation.rs`.
#[test]
fn format_stats_view_renders_counters_with_commas() {
    let result = realistic_stats_fixture();
    let rendered = format_stats_view(&result);
    assert!(rendered.contains("active: 1,342"));
    // 3-digit values should NOT grow a comma.
    assert!(rendered.contains("relations: 201"));
}

/// Guard: the `scope_tree` must render as a real indented tree, not as
/// the flat list the current `cx_stats` handler emits. Asserts both
/// the depth-0 and depth-2 indent shapes are present.
#[test]
fn format_stats_view_scope_tree_is_indented() {
    let result = realistic_stats_fixture();
    let rendered = format_stats_view(&result);
    // Depth 0 gets a 2-space indent.
    assert!(rendered.contains("\n  global"));
    // Depth 1 gets a 4-space indent.
    assert!(rendered.contains("\n    project:helioy"));
    // Depth 2 gets a 6-space indent.
    assert!(rendered.contains("\n      repo:context-matters"));
    assert!(rendered.contains("\n      repo:fmm"));
    // No orphans in the realistic fixture.
    assert!(!rendered.contains("orphaned scopes"));
}
