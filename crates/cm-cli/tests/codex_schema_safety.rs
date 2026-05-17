//! Regression guard for ALP-2476.
//!
//! OpenAI Codex's strict-mode tool validator runs before the model is ever
//! called and returns HTTP 400 for any parameter schema that exposes a
//! top-level `oneOf`, `anyOf`, `allOf`, or `not`, or `enum` without a
//! companion `type` (ref: openai/codex#2204, github/github-mcp-server#376,
//! LibreChat #5429). Gemini's tool pipeline enforces the same restriction.
//!
//! The whole class of failure mode is invisible to the model: it never sees
//! the prose, the redirect errors, or the examples. The schema is rejected
//! at the boundary. This test pins the boundary.
//!
//! Run after every change to `tool_contracts.rs` or `tools.toml`. If it
//! fails, the rejected pattern slipped back in — fix the emitter, not the
//! test.

use serde_json::Value;
use std::{fs, path::Path};

const SCHEMA_DIR: &str = "src/mcp/generated_schema";
const BANNED_TOP_LEVEL: [&str; 4] = ["oneOf", "anyOf", "allOf", "not"];

#[test]
fn generated_param_schemas_have_no_top_level_combinators() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SCHEMA_DIR);
    let entries = fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));

    let mut violations: Vec<String> = Vec::new();

    for entry in entries {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".json") {
            continue;
        }

        let content = fs::read_to_string(&path).expect("read schema");
        let schema: Value = serde_json::from_str(&content).expect("schema parses");

        let Some(props) = schema
            .get("inputSchema")
            .and_then(|v| v.get("properties"))
            .and_then(|v| v.as_object())
        else {
            continue;
        };

        for (param_name, param_schema) in props {
            for banned in BANNED_TOP_LEVEL {
                if param_schema.get(banned).is_some() {
                    violations.push(format!(
                        "{name}: param `{param_name}` has top-level `{banned}` (rejected by OpenAI Codex strict-mode and Gemini)"
                    ));
                }
            }

            if param_schema.get("enum").is_some() && param_schema.get("type").is_none() {
                violations.push(format!(
                    "{name}: param `{param_name}` has top-level `enum` without companion `type` (rejected by OpenAI strict-mode)"
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Codex-rejected schema patterns found in generated cx_* schemas:\n  {}\n\nFix the emitter in crates/cm-cli/src/tool_contracts.rs — do not relax this test. See ALP-2476.",
        violations.join("\n  ")
    );
}
