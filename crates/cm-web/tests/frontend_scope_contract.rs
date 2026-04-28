//! Source-level frontend contracts for the scope selector migration.
//!
//! The frontend currently has typecheck coverage rather than a JS test
//! runner. These checks lock the feed URL migration path without making
//! `cargo test -p cm-web` depend on Node.

use std::{fs, path::PathBuf};

fn frontend_source(relative: &str) -> String {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest.join("frontend/src").join(relative))
        .unwrap_or_else(|e| panic!("failed to read frontend source {relative}: {e}"))
}

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source marker {start:?}"));
    let tail = &source[start_index..];
    let end_index = tail
        .find(end)
        .unwrap_or_else(|| panic!("missing source marker {end:?} after {start:?}"));
    &tail[..end_index]
}

#[test]
fn feed_search_migrates_scope_path_url_state_to_scope() {
    let source = frontend_source("routes/feed/search.ts");

    assert!(
        source.contains("scope?: string;"),
        "FeedSearch should expose only scope as the public URL field"
    );
    assert!(
        !source.contains("scope_path?:"),
        "FeedSearch must not expose scope_path as public URL state"
    );
    assert!(
        source.contains("typeof search.scope === \"string\"")
            && source.contains("typeof search.scope_path === \"string\"")
            && source.contains("? search.scope_path"),
        "validateFeedSearch should migrate legacy scope_path URLs into scope"
    );
}

#[test]
fn frontend_api_contract_keeps_scope_path_as_rejected_input_only() {
    let source = frontend_source("api/scope-contract.test.ts");

    assert!(
        source.contains("api.entries.browse({ scope })")
            && source.contains("api.entries.search({ query: \"Scope\", scope })")
            && source.contains("api.export(scope)")
            && source.contains("api.entries.create(entry)")
            && source.contains("api.entries.merge("),
        "frontend request contracts should exercise scope on migrated surfaces"
    );
    assert!(
        source.matches("@ts-expect-error").count() >= 5,
        "frontend contract should keep type-level rejection coverage"
    );
}

#[test]
fn frontend_api_contract_exercises_cwd_inferred_recall_and_export() {
    let source = frontend_source("api/scope-contract.test.ts");

    assert!(
        source.contains("api.entries.recall({")
            && source.contains("api.agent.recall({")
            && source.contains("api.export({ scope: \"cwd_inferred\"")
            && source
                .matches("cwd: \"/tmp/helioy/context-matters\"")
                .count()
                >= 3,
        "frontend request contracts should exercise cwd_inferred recall and export with cwd"
    );
}

#[test]
fn frontend_api_client_serializes_cwd_query_params() {
    let source = frontend_source("api/client.ts");

    let entries_recall = source_between(
        &source,
        "recall(params: RecallParams): Promise<RecallView>",
        "get(id: string): Promise<EntryDetail>",
    );
    assert!(
        entries_recall.contains("scope: params.scope,")
            && entries_recall.contains("cwd: params.cwd,"),
        "entries.recall should serialize scope and cwd query params"
    );

    let agent_recall = source_between(
        &source,
        "agent: {\n    recall(params: RecallParams): Promise<RecallView>",
        "browse(params: AgentBrowseParams = {}): Promise<BrowseView>",
    );
    assert!(
        agent_recall.contains("scope: params.scope,") && agent_recall.contains("cwd: params.cwd,"),
        "agent.recall should serialize scope and cwd query params"
    );

    let export_call = source_between(&source, "export(params?: string | ExportParams)", "};");
    assert!(
        export_call.contains("toSearchParams({ scope: params?.scope, cwd: params?.cwd })"),
        "export should serialize scope and cwd query params"
    );
}
