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
        source.contains("api.entries.browse({ scope: subtreeScope })")
            && source.contains("api.entries.search({ query: \"Scope\", scope: subtreeScope })")
            && source.contains("api.agent.search({ query: \"Scope\", scope: setScope })")
            && source.contains("api.export({ scope: cwdScope })")
            && source.contains("api.entries.create(entry)")
            && source.contains("api.entries.merge("),
        "frontend request contracts should exercise scope on migrated surfaces"
    );
    assert!(
        source.matches("@ts-expect-error").count() >= 12,
        "frontend contract should keep type-level rejection coverage"
    );
}

#[test]
fn frontend_api_contract_exercises_scope_selector_variants() {
    let source = frontend_source("api/scope-contract.test.ts");

    assert!(
        source.contains("const pathScope: ScopeSelector = { kind: \"path\"")
            && source.contains("const cwdScope: ScopeSelector = {")
            && source.contains("kind: \"cwd_inferred\",")
            && source.contains("const subtreeScope: ScopeSelector = { kind: \"subtree\"")
            && source.contains("const setScope: ScopeSelector = {")
            && source.contains("kind: \"set\",")
            && source.contains("const allScope: ScopeSelector = { kind: \"all\" }"),
        "frontend request contracts should exercise every ScopeSelector variant"
    );
}

#[test]
fn frontend_api_client_serializes_structured_scope_query_params() {
    let source = frontend_source("api/client.ts");

    let entries_recall = source_between(
        &source,
        "recall(params: RecallParams): Promise<RecallView>",
        "get(id: string): Promise<EntryDetail>",
    );
    assert!(
        entries_recall.contains("scope: serializeScopeSelector(params.scope),")
            && !entries_recall.contains("cwd: params.cwd,"),
        "entries.recall should serialize one structured scope query param"
    );

    let agent_recall = source_between(
        &source,
        "agent: {\n    recall(params: RecallParams): Promise<RecallView>",
        "search(params: AgentSearchParams): Promise<RecallView>",
    );
    assert!(
        agent_recall.contains("scope: serializeScopeSelector(params.scope),")
            && !agent_recall.contains("cwd: params.cwd,"),
        "agent.recall should serialize one structured scope query param"
    );

    let agent_search = source_between(
        &source,
        "search(params: AgentSearchParams): Promise<RecallView>",
        "browse(params: AgentBrowseParams = {}): Promise<BrowseView>",
    );
    assert!(
        agent_search.contains("scope: serializeScopeSelector(params.scope),")
            && !agent_search.contains("cwd: params.cwd,"),
        "agent.search should serialize one structured scope query param"
    );

    let export_call = source_between(&source, "export(params?: ExportParams)", "};");
    assert!(
        export_call.contains("scope: serializeScopeSelector(params?.scope)")
            && !export_call.contains("cwd: params?.cwd"),
        "export should serialize one structured scope query param"
    );
}

#[test]
fn feed_search_scope_url_state_maps_to_all_selector_variants() {
    let source = frontend_source("routes/feed/search.ts");

    let converter = source_between(
        &source,
        "export function scopeSelectorFromFeedScope",
        "const ENTRY_KINDS",
    );

    assert!(
        converter.contains("scope === \"all\"")
            && converter.contains("scope.startsWith(\"path:\")")
            && converter.contains("scope.startsWith(\"subtree:\")")
            && converter.contains("scope.startsWith(\"set:\")")
            && converter.contains("scope.startsWith(\"cwd:\")"),
        "feed scope URL state should preserve every ScopeSelector prefix"
    );
    assert!(
        converter.contains(".filter(Boolean)")
            && converter.contains("paths.length > 0 ? { kind: \"set\", paths } : undefined"),
        "empty set URL state should not produce an empty set selector"
    );
    assert!(
        converter.contains("cwd ? { kind: \"cwd_inferred\", cwd } : { kind: \"cwd_inferred\" }")
            && converter.contains("return { kind: \"path\", path: scope };"),
        "cwd and unprefixed legacy URL state should keep their migration paths"
    );
}

#[test]
fn frontend_scope_controls_share_scope_selector_primitives() {
    let controls = frontend_source("components/domain/ScopeSelector.tsx");
    let hook = frontend_source("hooks/useScopeSelectorState.ts");
    let recall_bar = frontend_source("components/RecallBar.tsx");
    let filter_bar = frontend_source("components/FilterBar.tsx");
    let browse_pane = frontend_source("components/BrowsePane.tsx");

    assert!(
        controls.contains("import type { ScopeSelector as ScopeSelectorValue } from \"@/lib/scope\"")
            && controls.contains(
                "export type SingularScopeSelector = Extract<ScopeSelectorValue, { kind: \"path\" | \"cwd_inferred\" }>",
            ),
        "scope controls should share the value type from lib/scope and expose a singular type"
    );

    let scope_picker = source_between(
        &controls,
        "export function ScopePicker",
        "interface ScopeSelectorProps",
    );
    assert!(
        controls.contains("onChange: (scope: SingularScopeSelector | undefined) => void")
            && scope_picker.contains("{ kind: \"path\", path: nextValue }")
            && !scope_picker.contains("{ kind: \"subtree\"")
            && !scope_picker.contains("{ kind: \"set\"")
            && !scope_picker.contains("{ kind: \"all\""),
        "ScopePicker should emit only singular scope selectors"
    );

    let scope_selector = source_between(
        &controls,
        "export function ScopeSelector",
        "function modeFromScope",
    );
    assert!(
        scope_selector.contains("{ kind: \"path\", path }")
            && scope_selector.contains("{ kind: \"subtree\", path }")
            && scope_selector.contains("{ kind: \"path\", path: nextPath }")
            && scope_selector.contains("{ kind: \"subtree\", path: nextPath }")
            && scope_selector.contains("{ kind: \"set\", paths")
            && scope_selector.contains("{ kind: \"all\" }"),
        "ScopeSelector should produce every ScopeSelector variant"
    );
    assert!(
        scope_selector.contains(
            "nextPaths.length === 0 ? { kind: \"all\" } : { kind: \"set\", paths: nextPaths }"
        ) && scope_selector.contains("disabled={mode === \"all\"}"),
        "set mode should collapse empty selections to all and all mode should disable path selection"
    );

    assert!(
        hook.contains("useState<ScopeSelector | undefined>")
            && hook.contains("return [value, setValue]"),
        "useScopeSelectorState should be the shared ScopeSelector state hook"
    );

    assert!(
        recall_bar.contains("ScopePicker")
            && recall_bar.contains("useScopeSelectorState")
            && !recall_bar.contains("buildScopeOptions"),
        "RecallBar should use ScopePicker and stop building local scope options"
    );
    assert!(
        filter_bar.contains("ScopeSelector")
            && filter_bar.contains("useScopeSelectorState")
            && !filter_bar.contains("buildFacets(stats)")
            && !filter_bar.contains("{ key: \"scope\", placeholder: \"Scope\""),
        "FilterBar should use the full ScopeSelector primitive instead of a scope facet"
    );
    assert!(
        browse_pane.contains("useScopeSelectorState")
            && !browse_pane.contains("scope ? { kind: \"path\", path: scope } : undefined"),
        "BrowsePane should keep ScopeSelector state directly"
    );
}
