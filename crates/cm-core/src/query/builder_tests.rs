use super::QueryBuilder;
use crate::{EntryKind, ScopePath};

#[test]
fn query_builder_defaults() {
    let qb = QueryBuilder::new();
    assert!(qb.get_scope_path().is_none());
    assert!(qb.get_kinds().is_empty());
    assert!(!qb.get_include_superseded());
    assert!(qb.get_limit().is_none());
}

#[test]
fn query_builder_fluent() {
    let qb = QueryBuilder::new()
        .scope(ScopePath::global())
        .kinds(vec![EntryKind::Fact, EntryKind::Decision])
        .tag("rust")
        .created_by("agent:claude")
        .include_superseded(true)
        .limit(10);

    assert_eq!(qb.get_scope_path().unwrap().as_str(), "global");
    assert_eq!(qb.get_kinds().len(), 2);
    assert_eq!(qb.get_tag(), Some("rust"));
    assert_eq!(qb.get_created_by(), Some("agent:claude"));
    assert!(qb.get_include_superseded());
    assert_eq!(qb.get_limit(), Some(10));
}
