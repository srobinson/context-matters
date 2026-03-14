//! Build script — reads tools.toml and generates:
//!   src/mcp/generated_schema.rs  — MCP tool list JSON
//!   src/cli/generated_help.rs    — CLI help string constants
//!   templates/SKILL.md           — Claude Code skill documentation

use indexmap::IndexMap;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct ToolsToml {
    skill: Option<SkillConfig>,
    tools: IndexMap<String, ToolDef>,
}

#[derive(Deserialize)]
struct SkillConfig {
    workflow: String,
}

#[derive(Deserialize)]
struct ToolDef {
    cli_name: String,
    mcp_description: String,
    cli_about: String,
    #[serde(default)]
    params: Vec<ParamDef>,
}

#[derive(Deserialize)]
struct ParamDef {
    name: String,
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    required: bool,
    #[serde(rename = "enum")]
    enum_values: Option<Vec<String>>,
    mcp_description: String,
    cli_help: Option<String>,
    #[allow(dead_code)]
    cli_flag: Option<String>,
    /// For array params, the scalar type of each element (e.g. "string").
    /// When absent on an array param, the items schema is an inline object
    /// (currently only `exchanges` uses this).
    items_type: Option<String>,
}

fn main() {
    // tools.toml lives at workspace root, two levels up from cm-cli
    println!("cargo:rerun-if-changed=../../tools.toml");
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let tools_toml_path = Path::new(&manifest_dir).join("../../tools.toml");

    let content = fs::read_to_string(&tools_toml_path)
        .unwrap_or_else(|e| panic!("Failed to read tools.toml: {e}"));

    let parsed: ToolsToml =
        toml::from_str(&content).unwrap_or_else(|e| panic!("Failed to parse tools.toml: {e}"));

    // Generate all three outputs.
    let schema_rs = generate_mcp_schema(&parsed.tools);
    let help_rs = generate_cli_help(&parsed.tools);
    let skill_md = generate_skill_md(parsed.skill.as_ref(), &parsed.tools);

    write_if_changed(
        &Path::new(&manifest_dir).join("src/mcp/generated_schema.rs"),
        &schema_rs,
    );

    // Ensure cli directory exists for generated_help.rs
    let cli_dir = Path::new(&manifest_dir).join("src/cli");
    fs::create_dir_all(&cli_dir).unwrap_or_else(|e| panic!("Failed to create src/cli/: {e}"));

    write_if_changed(&cli_dir.join("generated_help.rs"), &help_rs);

    // Ensure templates directory exists for SKILL.md
    let templates_dir = Path::new(&manifest_dir).join("templates");
    fs::create_dir_all(&templates_dir)
        .unwrap_or_else(|e| panic!("Failed to create templates/: {e}"));

    write_if_changed(&templates_dir.join("SKILL.md"), &skill_md);
}

/// Only write if the content has changed to avoid spurious rebuilds.
fn write_if_changed(path: &Path, content: &str) {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content
    {
        return;
    }
    fs::write(path, content).unwrap_or_else(|e| panic!("Failed to write {}: {e}", path.display()));
}

// ---------------------------------------------------------------------------
// MCP schema generator
// ---------------------------------------------------------------------------

fn generate_mcp_schema(tools: &IndexMap<String, ToolDef>) -> String {
    let mut tool_jsons = Vec::new();

    for (tool_name, tool) in tools {
        let mut properties = serde_json::Map::new();
        let mut required: Vec<String> = Vec::new();

        for param in &tool.params {
            let mut prop = serde_json::Map::new();

            // Handle array types with items
            if param.type_ == "array" {
                prop.insert(
                    "type".to_string(),
                    serde_json::Value::String("array".to_string()),
                );
                let items_schema = match &param.items_type {
                    // Scalar array: items_type specifies the element type (e.g. "string")
                    Some(scalar) => serde_json::json!({"type": scalar}),
                    // Object array: no items_type means inline object schema.
                    // Currently only `exchanges` uses this pattern.
                    None => serde_json::json!({
                        "type": "object",
                        "properties": {
                            "user": {"type": "string"},
                            "assistant": {"type": "string"}
                        },
                        "required": ["user", "assistant"]
                    }),
                };
                prop.insert("items".to_string(), items_schema);
            } else {
                prop.insert(
                    "type".to_string(),
                    serde_json::Value::String(param.type_.clone()),
                );
            }

            prop.insert(
                "description".to_string(),
                serde_json::Value::String(param.mcp_description.clone()),
            );

            if let Some(ev) = &param.enum_values {
                prop.insert(
                    "enum".to_string(),
                    serde_json::Value::Array(
                        ev.iter()
                            .map(|s| serde_json::Value::String(s.clone()))
                            .collect(),
                    ),
                );
            }

            properties.insert(param.name.clone(), serde_json::Value::Object(prop));

            if param.required {
                required.push(param.name.clone());
            }
        }

        let mut input_schema = serde_json::json!({
            "type": "object",
            "properties": properties
        });
        if !required.is_empty() {
            input_schema["required"] = serde_json::Value::Array(
                required
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            );
        }

        tool_jsons.push(serde_json::json!({
            "name": tool_name,
            "description": tool.mcp_description,
            "inputSchema": input_schema
        }));
    }

    let json_val = serde_json::json!({ "tools": tool_jsons });
    let json_str = serde_json::to_string_pretty(&json_val).expect("JSON serialization failed");

    format!(
        "// AUTO-GENERATED by build.rs from tools.toml — do not edit\n\
         #![allow(clippy::all)]\n\
         #[rustfmt::skip]\n\
         \n\
         pub fn generated_tool_list() -> serde_json::Value {{\n\
             serde_json::from_str(r##\"{}\"##).expect(\"generated tool list is valid JSON\")\n\
         }}\n",
        json_str
    )
}

// ---------------------------------------------------------------------------
// CLI help constants generator
// ---------------------------------------------------------------------------

fn generate_cli_help(tools: &IndexMap<String, ToolDef>) -> String {
    let mut lines = vec![
        "// AUTO-GENERATED by build.rs from tools.toml — do not edit".to_string(),
        "#![allow(dead_code, unused)]".to_string(),
        "#![allow(clippy::all)]".to_string(),
    ];

    for tool in tools.values() {
        let prefix = tool.cli_name.to_uppercase().replace('-', "_");

        // Command-level about constant.
        let escaped = rust_escape(&tool.cli_about);
        lines.push("#[rustfmt::skip]".to_string());
        lines.push(format!("pub const {prefix}_ABOUT: &str = \"{escaped}\";"));

        // Per-param help constants.
        for param in &tool.params {
            if let Some(help) = &param.cli_help {
                let param_upper = param.name.to_uppercase().replace('-', "_");
                let escaped = rust_escape(help);
                lines.push("#[rustfmt::skip]".to_string());
                lines.push(format!(
                    "pub const {prefix}_{param_upper}_HELP: &str = \"{escaped}\";"
                ));
            }
        }

        lines.push(String::new());
    }

    lines.join("\n")
}

/// Escape a string for embedding in a Rust double-quoted string literal.
fn rust_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ---------------------------------------------------------------------------
// SKILL.md generator
// ---------------------------------------------------------------------------

fn generate_skill_md(skill: Option<&SkillConfig>, tools: &IndexMap<String, ToolDef>) -> String {
    let mut out = String::new();

    // Frontmatter
    out.push_str("---\n");
    out.push_str("name: cm\n");
    out.push_str("description: >\n");
    out.push_str("  Structured context store for AI agents. Use before any session to recall\n");
    out.push_str(
        "  relevant knowledge, and during work to persist facts, decisions, preferences,\n",
    );
    out.push_str(
        "  and lessons. All tools are prefixed `cx_*`. Trigger when: starting a session,\n",
    );
    out.push_str(
        "  discovering reusable knowledge, receiving user corrections, or ending a session\n",
    );
    out.push_str("  with conversation deposits.\n");
    out.push_str("---\n\n");

    // Introduction
    out.push_str("# Context Matters — Structured Context Store\n\n");
    out.push_str(
        "This project has a structured context store available via the **`cm` MCP server**. ",
    );
    out.push_str(
        "All tools are prefixed `cx_*`. Use them to persist and retrieve project knowledge ",
    );
    out.push_str("across sessions.\n\n");

    // MCP tools reference table
    out.push_str("## MCP Tools\n\n");
    out.push_str("| Tool | Purpose | Example |\n");
    out.push_str("|------|---------|--------|\n");

    let use_cases = [
        (
            "cx_recall",
            "Search and retrieve context relevant to the current task",
            r#"`cx_recall(query: "auth decisions", scope: "global/project:helioy")`"#,
        ),
        (
            "cx_store",
            "Persist a fact, decision, preference, or lesson",
            r#"`cx_store(title: "Use UUIDv7", body: "...", kind: "decision")`"#,
        ),
        (
            "cx_deposit",
            "Batch-store conversation exchanges",
            r#"`cx_deposit(exchanges: [{user: "...", assistant: "..."}])`"#,
        ),
        (
            "cx_browse",
            "List entries with filters and pagination",
            r#"`cx_browse(kind: "decision", scope_path: "global/project:helioy")`"#,
        ),
        (
            "cx_get",
            "Fetch full content for specific entry IDs",
            r#"`cx_get(ids: ["uuid1", "uuid2"])`"#,
        ),
        (
            "cx_update",
            "Partially update an existing entry",
            r#"`cx_update(id: "uuid", title: "Updated title")`"#,
        ),
        (
            "cx_forget",
            "Soft-delete entries no longer relevant",
            r#"`cx_forget(ids: ["uuid"])`"#,
        ),
        (
            "cx_stats",
            "View store statistics and scope breakdown",
            r#"`cx_stats()`"#,
        ),
        (
            "cx_export",
            "Export entries as JSON for backup",
            r#"`cx_export(scope_path: "global/project:helioy")`"#,
        ),
    ];

    for (tool_name, purpose, example) in &use_cases {
        out.push_str(&format!("| `{tool_name}` | {purpose} | {example} |\n"));
    }
    out.push('\n');

    // Workflow section from [skill].workflow
    if let Some(skill) = skill {
        out.push_str(skill.workflow.trim_start_matches('\n'));
        out.push('\n');
    }

    // Per-tool parameter reference tables
    out.push_str("\n## Parameter Reference\n\n");
    out.push_str("> Auto-generated from tools.toml.\n\n");

    for (tool_name, tool) in tools {
        out.push_str(&format!("### `{tool_name}`\n\n"));
        out.push_str(&format!("{}\n\n", tool.mcp_description));

        if !tool.params.is_empty() {
            out.push_str("| Parameter | Type | Required | Description |\n");
            out.push_str("|-----------|------|----------|-------------|\n");

            for param in &tool.params {
                let req = if param.required { "yes" } else { "no" };
                let type_str = if let Some(ev) = &param.enum_values {
                    let opts = ev.join(" \\| ");
                    format!("enum: {opts}")
                } else if param.type_ == "array" {
                    match &param.items_type {
                        Some(scalar) => format!("array<{scalar}>"),
                        None => "array<object>".to_string(),
                    }
                } else {
                    param.type_.clone()
                };
                // Truncate description for table readability
                let desc = if param.mcp_description.len() > 120 {
                    format!("{}...", &param.mcp_description[..117])
                } else {
                    param.mcp_description.clone()
                };
                out.push_str(&format!(
                    "| `{}` | {} | {} | {} |\n",
                    param.name, type_str, req, desc
                ));
            }
            out.push('\n');
        }
    }

    // Rules section
    out.push_str("## Rules\n\n");
    out.push_str("1. **Call `cx_recall` after receiving a task** with a summary of what you are working on\n");
    out.push_str("2. **Store selectively** — persist genuinely reusable knowledge, not routine observations\n");
    out.push_str(
        "3. **Classify accurately** — the `kind` field drives recall priority and filtering\n",
    );
    out.push_str("4. **Use specific scope paths** — overly broad scoping pollutes recall for unrelated work\n");
    out.push_str("5. **Two-phase retrieval** — `cx_recall`/`cx_browse` return snippets; use `cx_get` for full body\n");
    out.push_str("6. **Store feedback immediately** — when the user corrects you, `kind: \"feedback\"` gets highest recall priority\n");
    out.push_str("7. **Do not mention the context system** to the user unless asked\n");

    // CLI fallback section
    out.push_str("\n## CLI Fallback\n\n");
    out.push_str("If MCP tools are unavailable, use the CLI directly:\n\n");
    out.push_str("```bash\n");
    out.push_str("cm stats     # Show store statistics\n");
    out.push_str("cm serve     # Start MCP server on stdio\n");
    out.push_str("```\n");

    out
}
