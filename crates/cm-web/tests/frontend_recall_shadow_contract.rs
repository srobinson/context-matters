//! Source-level frontend contracts for the recall shadow dashboard panel.
//!
//! cm-web currently typechecks the frontend rather than running a JS
//! test runner. These checks lock the hook, panel states, and dashboard
//! wiring without making cargo tests depend on Node.

use std::{fs, path::PathBuf};

fn frontend_source(relative: &str) -> String {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest.join("frontend/src").join(relative))
        .unwrap_or_else(|e| panic!("failed to read frontend source {relative}: {e}"))
}

#[test]
fn recall_shadow_client_and_hook_contracts() {
    let client = frontend_source("api/client.ts");
    let hooks = frontend_source("api/hooks.ts");

    assert!(
        client.contains("import type { RecallShadowRow }")
            && client.contains("export interface RecallShadowListParams")
            && client.contains("recallShadow: {")
            && client
                .contains("list(params: RecallShadowListParams = {}): Promise<RecallShadowRow[]>")
            && client.contains("`/recall-shadow${toSearchParams({")
            && client.contains("routing: params.routing")
            && client.contains("scope_path: params.scope_path")
            && client.contains("top1_changed: params.top1_changed"),
        "API client should expose the typed recall shadow list endpoint"
    );

    assert!(
        hooks.contains("recallShadow: {")
            && hooks.contains("list: (params: RecallShadowListParams)")
            && hooks.contains("export function useRecallShadowHistory")
            && hooks.contains("queryFn: () => api.recallShadow.list(params)"),
        "React Query hook should reuse the central API client"
    );
}

#[test]
fn recall_shadow_panel_covers_render_states_and_links() {
    let panel = frontend_source("components/RecallShadowPanel.tsx");
    let dashboard = frontend_source("routes/index.tsx");

    assert!(
        panel.contains("useRecallShadowHistory(params)")
            && panel.contains("isLoading && <RecallShadowLoading />")
            && panel.contains("rows.length === 0")
            && panel.contains("No recall shadow rows yet")
            && panel.contains("rows.length > 0")
            && panel.contains("RecallShadowRowView"),
        "panel should cover loading, empty, and populated render states"
    );

    assert!(
        panel.contains("divergence")
            && panel.contains("avg overlap")
            && panel.contains("total rows")
            && panel.contains("routing")
            && panel.contains("scope path"),
        "panel should surface metrics and routing plus scope filters"
    );

    assert!(
        panel.contains("to=\"/feed\"")
            && panel.contains("entry_id: id")
            && dashboard.contains("import { RecallShadowPanel }")
            && dashboard.contains("<RecallShadowPanel />"),
        "diff IDs should link to the feed entry route and the dashboard should render the panel"
    );
}
