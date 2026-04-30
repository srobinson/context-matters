mod common;

use std::{
    collections::BTreeMap,
    fmt,
    sync::{Arc, Mutex},
};

use cm_capabilities::{
    ContentSearchRequest,
    recall::{RecallRequest, recall},
    scope::ScopeSelector,
    search::search,
};
use cm_core::{CmError, EntryKind, ScopeFilter, ScopePath};
use common::{create_global, seed_entry, test_store};
use tracing::{Event, Subscriber};
use tracing_subscriber::{Layer, layer::Context, prelude::*};

#[derive(Clone, Default)]
struct EventLog {
    events: Arc<Mutex<Vec<BTreeMap<String, String>>>>,
}

impl EventLog {
    fn take(&self) -> Vec<BTreeMap<String, String>> {
        std::mem::take(&mut *self.events.lock().unwrap())
    }
}

impl<S> Layer<S> for EventLog
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if !event.metadata().target().starts_with("cm_capabilities") {
            return;
        }

        let mut fields = BTreeMap::new();
        fields.insert("level".to_owned(), event.metadata().level().to_string());
        fields.insert("target".to_owned(), event.metadata().target().to_owned());
        event.record(&mut FieldVisitor {
            fields: &mut fields,
        });

        if fields.contains_key("op") {
            self.events.lock().unwrap().push(fields);
        }
    }
}

struct FieldVisitor<'a> {
    fields: &'a mut BTreeMap<String, String>,
}

impl tracing::field::Visit for FieldVisitor<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), value.to_owned());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}"));
    }
}

fn install_event_log() -> (EventLog, tracing::subscriber::DefaultGuard) {
    let log = EventLog::default();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::DEBUG)
        .with(log.clone());
    let guard = tracing::subscriber::set_default(subscriber);
    (log, guard)
}

fn event_for<'a>(events: &'a [BTreeMap<String, String>], op: &str) -> &'a BTreeMap<String, String> {
    events
        .iter()
        .find(|event| event.get("op").is_some_and(|value| value == op))
        .unwrap_or_else(|| panic!("missing event for op {op}: {events:#?}"))
}

fn assert_shared_schema(event: &BTreeMap<String, String>) {
    for field in [
        "op",
        "scope_variant",
        "scope_paths",
        "query_len",
        "result_count",
        "rank_source",
        "duration_ms",
        "error_variant",
    ] {
        assert!(
            event.contains_key(field),
            "missing field {field}: {event:#?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recall_logs_structured_completion_event() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Recall logging note",
        "alpha recall logging body",
        EntryKind::Fact,
    )
    .await;
    let (log, _guard) = install_event_log();

    let result = recall(
        &store,
        RecallRequest {
            query: Some("alpha".to_owned()),
            scope: Some(ScopeSelector::Path(ScopePath::global())),
            limit: 10,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(result.entries.len(), 1);
    let events = log.take();
    let event = event_for(&events, "recall");
    assert_shared_schema(event);
    assert_eq!(event.get("level").unwrap(), "DEBUG");
    assert_eq!(event.get("scope_variant").unwrap(), "path");
    assert_eq!(event.get("scope_paths").unwrap(), "[\"global\"]");
    assert_eq!(event.get("query_len").unwrap(), "5");
    assert_eq!(event.get("result_count").unwrap(), "1");
    assert_eq!(event.get("rank_source").unwrap(), "fts");
    assert_eq!(event.get("error_variant").unwrap(), "none");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_logs_matching_structured_completion_event() {
    let (store, _dir) = test_store().await;
    create_global(&store).await;
    seed_entry(
        &store,
        "Search logging note",
        "alpha search logging body",
        EntryKind::Fact,
    )
    .await;
    let (log, _guard) = install_event_log();

    let page = search(
        &store,
        ContentSearchRequest {
            query: "alpha".to_owned(),
            scope: ScopeFilter::All,
            kinds: None,
            tags: None,
            limit: 10,
            cursor: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(page.items.len(), 1);
    let events = log.take();
    let event = event_for(&events, "search");
    assert_shared_schema(event);
    assert_eq!(event.get("level").unwrap(), "DEBUG");
    assert_eq!(event.get("scope_variant").unwrap(), "all");
    assert_eq!(event.get("scope_paths").unwrap(), "[]");
    assert_eq!(event.get("query_len").unwrap(), "5");
    assert_eq!(event.get("result_count").unwrap(), "1");
    assert_eq!(event.get("rank_source").unwrap(), "fts");
    assert_eq!(event.get("error_variant").unwrap(), "none");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recall_logs_error_variant_on_failure() {
    let (store, _dir) = test_store().await;
    let (log, _guard) = install_event_log();

    let err = recall(
        &store,
        RecallRequest {
            query: Some("alpha".to_owned()),
            scope: Some(ScopeSelector::All),
            limit: 10,
            ..Default::default()
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(err, CmError::InvalidOperationInput { .. }));
    let events = log.take();
    let event = event_for(&events, "recall");
    assert_shared_schema(event);
    assert_eq!(event.get("scope_variant").unwrap(), "all");
    assert_eq!(event.get("scope_paths").unwrap(), "[]");
    assert_eq!(event.get("result_count").unwrap(), "0");
    assert_eq!(event.get("rank_source").unwrap(), "no_match");
    assert_eq!(
        event.get("error_variant").unwrap(),
        "invalid_operation_input"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_logs_error_variant_on_failure() {
    let (store, _dir) = test_store().await;
    let (log, _guard) = install_event_log();

    let err = search(
        &store,
        ContentSearchRequest {
            query: "   ".to_owned(),
            scope: ScopeFilter::All,
            kinds: None,
            tags: None,
            limit: 10,
            cursor: None,
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(err, CmError::InvalidOperationInput { .. }));
    let events = log.take();
    let event = event_for(&events, "search");
    assert_shared_schema(event);
    assert_eq!(event.get("scope_variant").unwrap(), "all");
    assert_eq!(event.get("scope_paths").unwrap(), "[]");
    assert_eq!(event.get("result_count").unwrap(), "0");
    assert_eq!(event.get("rank_source").unwrap(), "no_match");
    assert_eq!(
        event.get("error_variant").unwrap(),
        "invalid_operation_input"
    );
}
