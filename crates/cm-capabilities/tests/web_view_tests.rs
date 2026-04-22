//! Structural tests for `cm_capabilities::projection::web_view`.
//!
//! The web projection mirrors the YAML `format_browse_view` /
//! `format_recall_view` output as serialisable structs for the cm-web
//! Curator UI. These tests pin the four decisions from the ALP-1751
//! spec that matter for the frontend contract, independent of the YAML
//! wire shape:
//!
//!   1. Browse view hoists a uniform scope into the header.
//!   2. Browse view leaves mixed scopes on rows when they differ.
//!   3. Recall view brackets query matches with `«…»` on `Search` routing.
//!   4. Recall view populates the kinds / tags histograms.
//!
//! No SQLite store is involved. The projection functions are pure so
//! every fixture is built inline, matching the precedent in
//! `browse_format_tests.rs` / `recall_format_tests.rs`.

use std::collections::HashMap;

use chrono::{DateTime, Duration, TimeZone, Utc};
use uuid::Uuid;

use cm_capabilities::browse::BrowseResult;
use cm_capabilities::projection::{
    RecallRow, WebBrowseView, WebRecallView, project_web_browse_at, project_web_recall_at,
    project_web_update,
};
use cm_capabilities::recall::{
    RECALL_SCOPE_DEFAULT_ADVISORY, RecallAdvisory, RecallRequest, RecallResult, RecallRouting,
    SearchTier,
};
use cm_capabilities::scope::{
    BrowseScopeMode, ScopeResolution, ScopeResolutionCandidate, ScopeResolutionConfidence,
};
use cm_capabilities::update::UpdateResult;
use cm_core::{BrowseSort, Entry, EntryKind, EntryMeta, ScopePath};

/// Pinned reference `now`. Matches the value used in the sibling
/// format_tests files so the whole suite's `age:` fields stay
/// interpretable against a single instant.
fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 11, 12, 0, 0).unwrap()
}

#[test]
fn web_update_view_projects_ack_fields() {
    let result = UpdateResult {
        updated_id: "019d79d3-0000-7000-8000-000000000099".to_owned(),
        content_hash: "a".repeat(64),
    };

    let view = project_web_update(&result);

    assert_eq!(view.updated, result.updated_id);
    assert_eq!(view.content_hash, result.content_hash);
}

#[allow(clippy::too_many_arguments)]
fn make_entry(
    id_hex: &str,
    kind: EntryKind,
    title: &str,
    body: &str,
    scope: &str,
    tags: &[&str],
    updated_at: DateTime<Utc>,
) -> Entry {
    Entry {
        id: Uuid::parse_str(id_hex).expect("test fixture uuid parses"),
        scope_path: ScopePath::parse(scope).expect("test fixture scope parses"),
        kind,
        title: title.to_owned(),
        body: body.to_owned(),
        content_hash: "0".repeat(64),
        meta: Some(EntryMeta {
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            ..Default::default()
        }),
        created_by: "agent:claude-code".to_owned(),
        created_at: updated_at,
        updated_at,
        superseded_by: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn make_row(
    id_hex: &str,
    kind: EntryKind,
    title: &str,
    body: &str,
    scope: &str,
    tags: &[&str],
    updated_at: DateTime<Utc>,
    score: Option<f32>,
) -> RecallRow {
    RecallRow {
        entry: make_entry(id_hex, kind, title, body, scope, tags, updated_at),
        score,
    }
}

fn smart_scope_resolution_fixture() -> ScopeResolution {
    ScopeResolution {
        requested_scope: "auto".to_owned(),
        resolved_scope: ScopePath::parse("global/project:helioy/repo:context-matters")
            .expect("test fixture scope parses"),
        scope_mode: BrowseScopeMode::Resolved,
        confidence: ScopeResolutionConfidence::High,
        candidates: vec![
            ScopeResolutionCandidate {
                scope: ScopePath::parse("global/project:helioy/repo:context-matters")
                    .expect("test fixture scope parses"),
                score: 330,
                matched: vec![
                    "repo".to_owned(),
                    "project_parent".to_owned(),
                    "specificity".to_owned(),
                ],
            },
            ScopeResolutionCandidate {
                scope: ScopePath::parse("global/project:helioy")
                    .expect("test fixture scope parses"),
                score: 110,
                matched: vec!["project_parent".to_owned(), "project".to_owned()],
            },
        ],
        signals: vec![
            "cwd basename matched repo scope segment: context-matters".to_owned(),
            "cwd parent basename matched project scope segment: helioy".to_owned(),
        ],
    }
}

/// The happy path for scope hoisting: three rows at the same scope
/// must collapse into a single header label, and every row's own
/// `scope` field must be `None` afterwards. This is the signal the
/// frontend uses to render one scope chip above the list instead of
/// repeating it on every row.
#[test]
fn web_browse_view_hoists_uniform_scope() {
    let now = fixed_now();
    let entries = vec![
        make_entry(
            "019d79d3-0000-7000-8000-000000000001",
            EntryKind::Observation,
            "First",
            "Alpha body",
            "global/project:helioy",
            &["alpha"],
            now - Duration::hours(1),
        ),
        make_entry(
            "019d79d3-0000-7000-8000-000000000002",
            EntryKind::Observation,
            "Second",
            "Beta body",
            "global/project:helioy",
            &["beta"],
            now - Duration::hours(2),
        ),
        make_entry(
            "019d79d3-0000-7000-8000-000000000003",
            EntryKind::Observation,
            "Third",
            "Gamma body",
            "global/project:helioy",
            &["alpha", "beta"],
            now - Duration::hours(3),
        ),
    ];
    let result = BrowseResult {
        entries,
        total: 3,
        next_cursor: None,
        has_more: false,
        scope_used: None,
        include_resolution: false,
        limit_used: 50,
        sort_used: BrowseSort::Recent,
        relation_counts: HashMap::new(),
        resolution: None,
        advisory: None,
    };

    let view: WebBrowseView = project_web_browse_at(&result, now);

    assert!(
        view.resolution.is_none(),
        "legacy browse projection should omit resolution metadata",
    );
    assert_eq!(
        view.header.scope.as_deref(),
        Some("global/project:helioy"),
        "header should hoist the shared scope",
    );
    assert_eq!(view.entries.len(), 3);
    for row in &view.entries {
        assert!(
            row.scope.is_none(),
            "row {} leaked scope after hoist: {:?}",
            row.id,
            row.scope,
        );
    }
}

#[test]
fn web_browse_view_projects_scope_resolution() {
    let now = fixed_now();
    let entries = vec![make_entry(
        "019d79d3-0000-7000-8000-000000000001",
        EntryKind::Decision,
        "Smart browse scope",
        "Browse resolved to the local repo scope.",
        "global/project:helioy/repo:context-matters",
        &["smart-browse"],
        now - Duration::hours(1),
    )];
    let result = BrowseResult {
        entries,
        total: 1,
        next_cursor: None,
        has_more: false,
        scope_used: Some("auto".to_owned()),
        include_resolution: true,
        limit_used: 50,
        sort_used: BrowseSort::Recent,
        relation_counts: HashMap::new(),
        resolution: Some(smart_scope_resolution_fixture()),
        advisory: None,
    };

    let view: WebBrowseView = project_web_browse_at(&result, now);
    let resolution = view
        .resolution
        .expect("smart browse projection should include resolution metadata");

    assert_eq!(resolution.requested_scope, "auto");
    assert_eq!(
        resolution.resolved_scope,
        "global/project:helioy/repo:context-matters",
    );
    assert_eq!(resolution.scope_mode, "resolved");
    assert_eq!(resolution.confidence, "high");
    assert_eq!(resolution.candidates.len(), 2);
    assert_eq!(
        resolution.candidates[0].scope,
        "global/project:helioy/repo:context-matters",
    );
    assert_eq!(resolution.candidates[0].score, 330);
    assert_eq!(
        resolution.candidates[0].matched,
        vec![
            "repo".to_owned(),
            "project_parent".to_owned(),
            "specificity".to_owned(),
        ],
    );
    assert_eq!(
        resolution.signals,
        vec![
            "cwd basename matched repo scope segment: context-matters".to_owned(),
            "cwd parent basename matched project scope segment: helioy".to_owned(),
        ],
    );
}

/// The safety case for scope hoisting: when rows live at different
/// scopes the header must NOT hoist anything, and each row must
/// carry its own `Some(scope)`. A frontend that trusted the header
/// without reading rows would otherwise mis-label every entry.
#[test]
fn web_browse_view_mixed_scope() {
    let now = fixed_now();
    let entries = vec![
        make_entry(
            "019d79d3-0000-7000-8000-000000000001",
            EntryKind::Observation,
            "At global",
            "Body one",
            "global",
            &[],
            now - Duration::hours(1),
        ),
        make_entry(
            "019d79d3-0000-7000-8000-000000000002",
            EntryKind::Observation,
            "At project",
            "Body two",
            "global/project:helioy",
            &[],
            now - Duration::hours(2),
        ),
    ];
    let result = BrowseResult {
        entries,
        total: 2,
        next_cursor: None,
        has_more: false,
        scope_used: None,
        include_resolution: false,
        limit_used: 50,
        sort_used: BrowseSort::Recent,
        relation_counts: HashMap::new(),
        resolution: None,
        advisory: None,
    };

    let view: WebBrowseView = project_web_browse_at(&result, now);

    assert!(
        view.header.scope.is_none(),
        "header must not hoist mixed scopes, got {:?}",
        view.header.scope,
    );
    assert_eq!(view.entries[0].scope.as_deref(), Some("global"));
    assert_eq!(
        view.entries[1].scope.as_deref(),
        Some("global/project:helioy"),
    );
}

/// `Search`-routed recall with a non-empty query must bracket every
/// query match inside the row snippet with `«…»` guillemets. This is
/// the only per-row signal the frontend has that a given entry matched
/// a specific term inside the body, so the gate (is_search AND
/// query.is_some()) is the load-bearing behaviour for this test.
#[test]
fn web_recall_view_brackets_snippets_on_search() {
    let now = fixed_now();
    let entries = vec![make_row(
        "019d79d3-0000-7000-8000-000000000001",
        EntryKind::Decision,
        "Snippet strategy",
        "We centre the snippet window on the first query-term match \
         so the reader sees the matched span in context.",
        "global",
        &["projection"],
        now - Duration::hours(1),
        Some(-2.0),
    )];
    let result = RecallResult {
        entries,
        scope_chain: vec!["global".to_owned()],
        scope_hits: vec![("global".to_owned(), 1)],
        token_estimate: 100,
        routing: RecallRouting::Search,
        tier: Some(SearchTier::Exact),
        candidates_before_filter: 1,
        fetch_limit_used: 50,
        relation_counts: HashMap::new(),
        advisories: Vec::new(),
    };
    let request = RecallRequest {
        query: Some("snippet".to_owned()),
        limit: 50,
        ..Default::default()
    };

    let view: WebRecallView = project_web_recall_at(&result, &request, now);

    assert_eq!(view.entries.len(), 1);
    let snippet = &view.entries[0].snippet;
    assert!(
        snippet.contains("«snippet»"),
        "expected bracketed match, got: {snippet}",
    );
}

/// The header's kinds / tags histograms must reflect the full returned
/// slice exactly. The frontend renders the distribution widgets
/// directly off these counts, so any drift between the projection and
/// the underlying rows would show up as a wrong number above the list.
/// Uses a non-`Search` routing so the test is not entangled with the
/// bracketing gate exercised in the previous case.
#[test]
fn web_recall_view_histograms_populated() {
    let now = fixed_now();
    let entries = vec![
        make_row(
            "019d79d3-0000-7000-8000-000000000001",
            EntryKind::Decision,
            "First",
            "Body one",
            "global/project:helioy",
            &["alpha", "beta"],
            now - Duration::hours(1),
            None,
        ),
        make_row(
            "019d79d3-0000-7000-8000-000000000002",
            EntryKind::Decision,
            "Second",
            "Body two",
            "global/project:helioy",
            &["alpha"],
            now - Duration::hours(2),
            None,
        ),
        make_row(
            "019d79d3-0000-7000-8000-000000000003",
            EntryKind::Lesson,
            "Third",
            "Body three",
            "global",
            &["gamma"],
            now - Duration::hours(3),
            None,
        ),
    ];
    let result = RecallResult {
        entries,
        scope_chain: vec!["global/project:helioy".to_owned(), "global".to_owned()],
        scope_hits: vec![
            ("global/project:helioy".to_owned(), 2),
            ("global".to_owned(), 1),
        ],
        token_estimate: 300,
        routing: RecallRouting::ScopeResolve,
        tier: None,
        candidates_before_filter: 3,
        fetch_limit_used: 50,
        relation_counts: HashMap::new(),
        advisories: Vec::new(),
    };
    let request = RecallRequest {
        query: None,
        limit: 50,
        ..Default::default()
    };

    let view: WebRecallView = project_web_recall_at(&result, &request, now);

    assert_eq!(view.header.kinds_histogram.get("decision"), Some(&2));
    assert_eq!(view.header.kinds_histogram.get("lesson"), Some(&1));
    assert_eq!(view.header.kinds_histogram.len(), 2);

    assert_eq!(view.header.tags_histogram.get("alpha"), Some(&2));
    assert_eq!(view.header.tags_histogram.get("beta"), Some(&1));
    assert_eq!(view.header.tags_histogram.get("gamma"), Some(&1));
    assert_eq!(view.header.tags_histogram.len(), 3);
}

#[test]
fn web_recall_view_projects_capability_advisories() {
    let now = fixed_now();
    let result = RecallResult {
        entries: Vec::new(),
        scope_chain: vec!["global".to_owned()],
        scope_hits: Vec::new(),
        token_estimate: 0,
        routing: RecallRouting::ScopeResolve,
        tier: None,
        candidates_before_filter: 0,
        fetch_limit_used: 20,
        relation_counts: HashMap::new(),
        advisories: vec![RecallAdvisory::ScopeDefaulted {
            applied: "global".to_owned(),
        }],
    };
    let request = RecallRequest {
        limit: 20,
        ..Default::default()
    };

    let view: WebRecallView = project_web_recall_at(&result, &request, now);

    assert_eq!(view.advisories, vec![RECALL_SCOPE_DEFAULT_ADVISORY]);
}
