use cm_core::*;

// ── Acceptance Criteria 1-9: ScopePath parse/reject ────────────────

#[test]
fn criterion_01_parse_global() {
    let path = ScopePath::parse("global").unwrap();
    assert_eq!(path.leaf_kind(), ScopeKind::Global);
    assert_eq!(path.depth(), 1);
}

#[test]
fn criterion_02_parse_project_repo() {
    let path = ScopePath::parse("global/project:helioy/repo:nancyr").unwrap();
    assert_eq!(path.leaf_kind(), ScopeKind::Repo);
    assert_eq!(path.depth(), 3);
}

#[test]
fn criterion_03_skipped_hierarchy() {
    let path = ScopePath::parse("global/project:helioy/session:deploy-review").unwrap();
    assert_eq!(path.leaf_kind(), ScopeKind::Session);
    assert_eq!(path.depth(), 3);
}

#[test]
fn scope_path_allows_nested_projects() {
    let path = ScopePath::parse("global/project:helioy/project:agents/repo:nancyr").unwrap();
    assert_eq!(path.leaf_kind(), ScopeKind::Repo);
    assert_eq!(path.depth(), 4);
    let ancestors: Vec<&str> = path.ancestors().collect();
    assert_eq!(
        ancestors,
        vec![
            "global/project:helioy/project:agents/repo:nancyr",
            "global/project:helioy/project:agents",
            "global/project:helioy",
            "global",
        ]
    );
}

#[test]
fn criterion_04_missing_global_root() {
    let err = ScopePath::parse("project:helioy").unwrap_err();
    assert!(
        matches!(err, ScopePathError::MissingGlobalRoot),
        "expected MissingGlobalRoot, got: {err}"
    );
}

#[test]
fn criterion_05_invalid_kind() {
    let err = ScopePath::parse("global/workspace:foo").unwrap_err();
    assert!(
        matches!(err, ScopePathError::InvalidKind(_)),
        "expected InvalidKind, got: {err}"
    );
}

#[test]
fn criterion_06_empty_path() {
    let err = ScopePath::parse("").unwrap_err();
    assert!(
        matches!(err, ScopePathError::Empty),
        "expected Empty, got: {err}"
    );
}

#[test]
fn criterion_07_out_of_order() {
    let err = ScopePath::parse("global/repo:x/project:y").unwrap_err();
    assert!(
        matches!(err, ScopePathError::OutOfOrder { .. }),
        "expected OutOfOrder, got: {err}"
    );
}

#[test]
fn criterion_08_uppercase_identifier() {
    let err = ScopePath::parse("global/project:UPPER").unwrap_err();
    assert!(
        matches!(err, ScopePathError::InvalidIdentifier(_)),
        "expected InvalidIdentifier, got: {err}"
    );
}

#[test]
fn criterion_09_too_long() {
    let long = format!("global/project:{}", "x".repeat(250));
    let err = ScopePath::parse(&long).unwrap_err();
    assert!(
        matches!(err, ScopePathError::TooLong { .. }),
        "expected TooLong, got: {err}"
    );
}

// ── Acceptance Criteria 10-11: Ancestor traversal ──────────────────

#[test]
fn criterion_10_ancestors_three_levels() {
    let path = ScopePath::parse("global/project:helioy/repo:nancyr").unwrap();
    let ancestors: Vec<&str> = path.ancestors().collect();
    assert_eq!(
        ancestors,
        vec![
            "global/project:helioy/repo:nancyr",
            "global/project:helioy",
            "global",
        ]
    );
}

#[test]
fn criterion_11_ancestors_global_only() {
    let path = ScopePath::global();
    let ancestors: Vec<&str> = path.ancestors().collect();
    assert_eq!(ancestors, vec!["global"]);
}

// ── Acceptance Criteria 12-16: Content hashing ─────────────────────

fn make_new_entry(scope: &str, kind: EntryKind, title: &str, body: &str) -> NewEntry {
    NewEntry {
        scope_path: ScopePath::parse(scope).unwrap(),
        kind,
        title: title.to_string(),
        body: body.to_string(),
        created_by: "test:unit".to_string(),
        meta: None,
    }
}

#[test]
fn criterion_12_same_content_different_title_same_hash() {
    let a = make_new_entry("global", EntryKind::Fact, "Title A", "same body");
    let b = make_new_entry("global", EntryKind::Fact, "Title B", "same body");
    assert_eq!(a.content_hash(), b.content_hash());
}

#[test]
fn criterion_13_different_body_different_hash() {
    let a = make_new_entry("global", EntryKind::Fact, "Title", "body one");
    let b = make_new_entry("global", EntryKind::Fact, "Title", "body two");
    assert_ne!(a.content_hash(), b.content_hash());
}

#[test]
fn criterion_14_different_scope_different_hash() {
    let a = make_new_entry("global", EntryKind::Fact, "Title", "same body");
    let b = make_new_entry(
        "global/project:helioy",
        EntryKind::Fact,
        "Title",
        "same body",
    );
    assert_ne!(a.content_hash(), b.content_hash());
}

#[test]
fn criterion_15_different_kind_different_hash() {
    let a = make_new_entry("global", EntryKind::Fact, "Title", "same body");
    let b = make_new_entry("global", EntryKind::Decision, "Title", "same body");
    assert_ne!(a.content_hash(), b.content_hash());
}

#[test]
fn criterion_16_hash_format() {
    let entry = make_new_entry("global", EntryKind::Fact, "Title", "body");
    let hash = entry.content_hash();
    assert_eq!(hash.len(), 64, "hash should be 64 hex chars");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "hash should be hex"
    );
    assert_eq!(hash, hash.to_lowercase(), "hash should be lowercase");
}

// ── Acceptance Criteria 17-19: NewScope derivation ─────────────────

#[test]
fn criterion_17_new_scope_kind() {
    let ns = NewScope {
        path: ScopePath::parse("global/project:helioy").unwrap(),
        label: "Helioy".to_string(),
        meta: None,
    };
    assert_eq!(ns.kind(), ScopeKind::Project);
}

#[test]
fn criterion_18_new_scope_parent_path() {
    let ns = NewScope {
        path: ScopePath::parse("global/project:helioy").unwrap(),
        label: "Helioy".to_string(),
        meta: None,
    };
    let parent = ns.parent_path().unwrap();
    assert_eq!(parent.as_str(), "global");
}

#[test]
fn criterion_19_new_scope_global_no_parent() {
    let ns = NewScope {
        path: ScopePath::global(),
        label: "Global".to_string(),
        meta: None,
    };
    assert!(ns.parent_path().is_none());
}

// ── Acceptance Criteria 20-23: Serde round-trips ───────────────────

#[test]
fn criterion_20_entry_kind_serde() {
    let kind = EntryKind::Feedback;
    let json = serde_json::to_string(&kind).unwrap();
    assert_eq!(json, "\"feedback\"");
    let back: EntryKind = serde_json::from_str(&json).unwrap();
    assert_eq!(back, kind);

    // All variants round-trip
    for variant in [
        EntryKind::Fact,
        EntryKind::Decision,
        EntryKind::Preference,
        EntryKind::Lesson,
        EntryKind::Reference,
        EntryKind::Feedback,
        EntryKind::Pattern,
        EntryKind::Observation,
    ] {
        let s = serde_json::to_string(&variant).unwrap();
        let back: EntryKind = serde_json::from_str(&s).unwrap();
        assert_eq!(back, variant);
    }
}

#[test]
fn criterion_21_entry_meta_extra_fields_roundtrip() {
    let json = r#"{
        "tags": ["rust", "tokio"],
        "confidence": "high",
        "custom_field": 42,
        "nested": {"key": "value"}
    }"#;

    let meta: EntryMeta = serde_json::from_str(json).unwrap();
    assert_eq!(meta.tags, vec!["rust", "tokio"]);
    assert_eq!(meta.confidence, Some(Confidence::High));
    assert!(meta.extra.contains_key("custom_field"));
    assert!(meta.extra.contains_key("nested"));

    // Round-trip preserves extra fields
    let serialized = serde_json::to_string(&meta).unwrap();
    let back: EntryMeta = serde_json::from_str(&serialized).unwrap();
    assert_eq!(back.extra["custom_field"], 42);
}

#[test]
fn criterion_22_scope_path_serde_as_string() {
    let path = ScopePath::parse("global/project:helioy").unwrap();
    let json = serde_json::to_string(&path).unwrap();
    assert_eq!(json, "\"global/project:helioy\"");

    let back: ScopePath = serde_json::from_str(&json).unwrap();
    assert_eq!(back.as_str(), "global/project:helioy");
}

#[test]
fn criterion_23_entry_meta_none_skip() {
    // Entry with meta: None should not have "meta" key in JSON
    let entry_json = serde_json::json!({
        "id": "019536a0-0000-7000-0000-000000000001",
        "scope_path": "global",
        "kind": "fact",
        "title": "test",
        "body": "test body",
        "content_hash": "a".repeat(64),
        "created_by": "test:unit",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z"
    });

    let entry: Entry = serde_json::from_value(entry_json).unwrap();
    assert!(entry.meta.is_none());

    let serialized = serde_json::to_value(&entry).unwrap();
    assert!(
        !serialized.as_object().unwrap().contains_key("meta"),
        "meta should be skipped when None"
    );
}

// ── Acceptance Criteria 24-25: Error formatting ────────────────────

#[test]
fn criterion_24_entry_not_found_display() {
    let id = uuid::Uuid::nil();
    let err = CmError::EntryNotFound(id);
    assert_eq!(err.to_string(), format!("entry not found: {id}"));
}

#[test]
fn criterion_25_out_of_order_display() {
    let err = ScopePathError::OutOfOrder {
        got: "project".to_string(),
        after: "repo".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "scope kind 'project' cannot appear after 'repo'"
    );
}

// ── Additional edge cases ──────────────────────────────────────────

#[test]
fn scope_path_single_char_identifier() {
    let path = ScopePath::parse("global/project:x").unwrap();
    assert_eq!(path.leaf_kind(), ScopeKind::Project);
}

#[test]
fn scope_path_hyphenated_identifier() {
    let path = ScopePath::parse("global/project:my-project").unwrap();
    assert_eq!(path.as_str(), "global/project:my-project");
}

#[test]
fn scope_path_trailing_hyphen_rejected() {
    let err = ScopePath::parse("global/project:bad-").unwrap_err();
    assert!(matches!(err, ScopePathError::InvalidIdentifier(_)));
}

#[test]
fn scope_path_leading_hyphen_rejected() {
    let err = ScopePath::parse("global/project:-bad").unwrap_err();
    assert!(matches!(err, ScopePathError::InvalidIdentifier(_)));
}

#[test]
fn scope_path_malformed_segment() {
    let err = ScopePath::parse("global/nocolon").unwrap_err();
    assert!(matches!(err, ScopePathError::MalformedSegment(_)));
}

#[test]
fn scope_path_full_depth() {
    let path = ScopePath::parse("global/project:helioy/repo:nancyr/session:abc123").unwrap();
    assert_eq!(path.depth(), 4);
    assert_eq!(path.leaf_kind(), ScopeKind::Session);
    let ancestors: Vec<&str> = path.ancestors().collect();
    assert_eq!(ancestors.len(), 4);
}

#[test]
fn relation_kind_roundtrip() {
    for kind in [
        RelationKind::Supersedes,
        RelationKind::RelatesTo,
        RelationKind::Contradicts,
        RelationKind::Elaborates,
        RelationKind::DependsOn,
    ] {
        let s = serde_json::to_string(&kind).unwrap();
        let back: RelationKind = serde_json::from_str(&s).unwrap();
        assert_eq!(back, kind);
    }
}

#[test]
fn pagination_defaults() {
    let p = Pagination::default();
    assert_eq!(p.limit, 50);
    assert!(p.cursor.is_none());
}

#[test]
fn entry_filter_defaults() {
    let f = EntryFilter::default();
    assert!(f.scope.is_none());
    assert!(f.kind.is_none());
    assert!(!f.include_superseded);
    assert_eq!(f.pagination.limit, 50);
}

#[test]
fn confidence_serde() {
    let json = "\"medium\"";
    let c: Confidence = serde_json::from_str(json).unwrap();
    assert_eq!(c, Confidence::Medium);
}

// ── Recall ranking ─────────────────────────────────────────────────

fn rank_entry(
    kind: EntryKind,
    confidence: Option<Confidence>,
    priority: Option<i32>,
    scope: &str,
    updated_at: &str,
    id: &str,
) -> Entry {
    let timestamp = updated_at.parse().unwrap();
    Entry {
        id: uuid::Uuid::parse_str(id).unwrap(),
        scope_path: ScopePath::parse(scope).unwrap(),
        kind,
        title: "rank test".to_owned(),
        body: "rank body".to_owned(),
        content_hash: "hash".to_owned(),
        meta: Some(EntryMeta {
            confidence,
            priority,
            ..Default::default()
        }),
        created_by: "test:rank".to_owned(),
        created_at: timestamp,
        updated_at: timestamp,
        superseded_by: None,
    }
}

#[test]
fn recall_rank_key_orders_kind_confidence_priority_scope_prefix() {
    let base_time = "2026-06-16T00:00:00Z";
    let low_id = "01890f99-0000-7000-8000-000000000001";
    let high_id = "01890f99-0000-7000-8000-000000000002";

    let feedback = rank_entry(
        EntryKind::Feedback,
        Some(Confidence::Medium),
        Some(0),
        "global",
        base_time,
        low_id,
    );
    let fact = rank_entry(
        EntryKind::Fact,
        Some(Confidence::Medium),
        Some(0),
        "global",
        base_time,
        high_id,
    );
    assert!(recall_rank_key(&feedback) < recall_rank_key(&fact));
    assert_eq!(EntryKind::Feedback.rank_tier(), 0);

    assert!(Confidence::High.recall_rank() < Confidence::Medium.recall_rank());
    assert!(Confidence::Medium.recall_rank() < Confidence::Low.recall_rank());

    let medium = rank_entry(
        EntryKind::Decision,
        Some(Confidence::Medium),
        Some(0),
        "global",
        base_time,
        low_id,
    );
    let absent = rank_entry(
        EntryKind::Decision,
        None,
        Some(0),
        "global",
        base_time,
        low_id,
    );
    assert_eq!(recall_rank_key(&medium), recall_rank_key(&absent));

    let global_high_priority = rank_entry(
        EntryKind::Fact,
        Some(Confidence::Medium),
        Some(10),
        "global",
        base_time,
        low_id,
    );
    let local_default_priority = rank_entry(
        EntryKind::Fact,
        Some(Confidence::Medium),
        Some(0),
        "global/project:helioy/repo:context-matters",
        base_time,
        high_id,
    );
    assert!(recall_rank_key(&global_high_priority) < recall_rank_key(&local_default_priority));
}

#[test]
fn recall_rank_key_prefix_order_is_transitive() {
    let kinds = [
        EntryKind::Feedback,
        EntryKind::Decision,
        EntryKind::Preference,
        EntryKind::Lesson,
        EntryKind::Pattern,
        EntryKind::Fact,
        EntryKind::Reference,
        EntryKind::Observation,
    ];
    let confidences = [
        Some(Confidence::High),
        Some(Confidence::Medium),
        Some(Confidence::Low),
    ];
    let priorities = [1, 0];
    let scopes = ["global/project:helioy/repo:context-matters", "global"];

    let mut entries = Vec::new();
    let mut id_suffix = 1u128;
    for kind in kinds {
        for confidence in confidences {
            for priority in priorities {
                for scope in scopes {
                    entries.push(rank_entry(
                        kind,
                        confidence,
                        Some(priority),
                        scope,
                        "2026-06-16T00:00:00Z",
                        &format!("01890f99-0000-7000-8000-{id_suffix:012x}"),
                    ));
                    id_suffix += 1;
                }
            }
        }
    }

    let keys = entries.iter().map(recall_rank_key).collect::<Vec<_>>();
    for (left_index, left) in keys.iter().enumerate() {
        assert!(!(left < left), "rank key ordering must be irreflexive");
        for (right_index, right) in keys.iter().enumerate() {
            if left_index != right_index {
                assert_ne!(left, right, "distinct entries must have distinct rank keys");
            }
            if left < right {
                assert!(!(right < left), "rank key ordering must be antisymmetric");
            }
        }
    }

    let mut sorted = keys;
    sorted.sort();
    for window in sorted.windows(3) {
        assert!(window[0] < window[1]);
        assert!(window[1] < window[2]);
        assert!(
            window[0] < window[2],
            "rank key ordering must be transitive"
        );
    }
}

#[test]
fn recall_ranking_mode_parse_is_pure_and_fail_closed() {
    assert_eq!(
        RecallRankingMode::parse("legacy"),
        Some(RecallRankingMode::Legacy)
    );
    assert_eq!(
        RecallRankingMode::parse("shadow"),
        Some(RecallRankingMode::Shadow)
    );
    assert_eq!(
        RecallRankingMode::parse("live"),
        Some(RecallRankingMode::Live)
    );
    assert_eq!(
        RecallRankingMode::parse_or_legacy("invalid"),
        RecallRankingMode::Legacy
    );
}

// ── Mutation types ─────────────────────────────────────────────────

#[test]
fn write_context_construction() {
    for source in [
        MutationSource::Mcp,
        MutationSource::Cli,
        MutationSource::Web,
        MutationSource::Helix,
    ] {
        let ctx = WriteContext::new(source);
        assert_eq!(ctx.source, source);
    }
}

#[test]
fn mutation_source_serde() {
    let cases = [
        (MutationSource::Mcp, "\"mcp\""),
        (MutationSource::Cli, "\"cli\""),
        (MutationSource::Web, "\"web\""),
        (MutationSource::Helix, "\"helix\""),
    ];
    for (variant, expected_json) in cases {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected_json);
        let back: MutationSource = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}

#[test]
fn mutation_action_serde() {
    let cases = [
        (MutationAction::Create, "\"create\""),
        (MutationAction::Update, "\"update\""),
        (MutationAction::Forget, "\"forget\""),
        (MutationAction::Supersede, "\"supersede\""),
    ];
    for (variant, expected_json) in cases {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, expected_json);
        let back: MutationAction = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}
