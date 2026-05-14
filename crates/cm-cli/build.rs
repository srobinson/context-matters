//! Build script: reads tools.toml and generates:
//!   src/mcp/generated_schema.rs  - MCP tool list JSON
//!   src/mcp/generated_instructions.rs - MCP server instructions
//!   src/cli/generated_help.rs    - CLI help string constants
//!   templates/SKILL.md           - Claude Code skill documentation
//!   ../../README.md              - public tool documentation

use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[allow(dead_code)]
#[path = "src/tool_contracts.rs"]
mod tool_contracts;
#[path = "src/tool_docs.rs"]
mod tool_docs;

use tool_contracts::{ToolContract, ToolContractRegistry};
use tool_docs::{
    render_generated_instructions_rs, render_readme_md, render_server_instructions, render_skill_md,
};

fn main() {
    // tools.toml lives at workspace root, two levels up from cm-cli
    println!("cargo:rerun-if-changed=../../tools.toml");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/tool_contracts.rs");
    println!("cargo:rerun-if-changed=src/tool_docs.rs");
    println!("cargo:rerun-if-env-changed=CONTEXT_MATTERS_GIT_SHA");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    emit_version();
    let tools_toml_path = Path::new(&manifest_dir).join("../../tools.toml");

    let content = fs::read_to_string(&tools_toml_path)
        .unwrap_or_else(|e| panic!("Failed to read tools.toml: {e}"));

    let registry = ToolContractRegistry::from_toml_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse tools.toml: {e}"));

    // Generate all contract-backed outputs.
    let (schema_rs, schema_files) = generate_mcp_schema(registry.tools());
    let help_rs = generate_cli_help(registry.tools());
    let skill_md = render_skill_md(registry.skill(), registry.tools());
    let instructions = render_server_instructions(registry.tools());
    let instructions_rs = render_generated_instructions_rs(&instructions);
    let readme_md = render_readme_md(registry.tools());

    write_if_changed(
        &Path::new(&manifest_dir).join("src/mcp/generated_schema.rs"),
        &schema_rs,
    );
    write_if_changed(
        &Path::new(&manifest_dir).join("src/mcp/generated_instructions.rs"),
        &instructions_rs,
    );
    let schema_dir = Path::new(&manifest_dir).join("src/mcp/generated_schema");
    fs::create_dir_all(&schema_dir)
        .unwrap_or_else(|e| panic!("Failed to create src/mcp/generated_schema/: {e}"));
    let mut expected_schema_files = HashSet::new();
    for (file_name, content) in &schema_files {
        expected_schema_files.insert(file_name.as_str());
        write_if_changed(&schema_dir.join(file_name), content);
    }
    remove_stale_generated_files(&schema_dir, &expected_schema_files);

    // Ensure cli directory exists for generated_help.rs
    let cli_dir = Path::new(&manifest_dir).join("src/cli");
    fs::create_dir_all(&cli_dir).unwrap_or_else(|e| panic!("Failed to create src/cli/: {e}"));

    write_if_changed(&cli_dir.join("generated_help.rs"), &help_rs);

    // Ensure templates directory exists for SKILL.md
    let templates_dir = Path::new(&manifest_dir).join("templates");
    fs::create_dir_all(&templates_dir)
        .unwrap_or_else(|e| panic!("Failed to create templates/: {e}"));

    write_if_changed(&templates_dir.join("SKILL.md"), &skill_md);
    write_if_changed(
        &Path::new(&manifest_dir).join("../../README.md"),
        &readme_md,
    );
}

fn emit_version() {
    let package_version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION not set");
    let version = match std::env::var("CONTEXT_MATTERS_GIT_SHA") {
        Ok(sha) if !sha.trim().is_empty() => format!("{package_version}+{}", sha.trim()),
        _ => package_version,
    };
    println!("cargo:rustc-env=CONTEXT_MATTERS_VERSION={version}");
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

fn remove_stale_generated_files(dir: &Path, expected: &HashSet<&str>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("Failed to read generated schema dir {}: {e}", dir.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|e| panic!("Failed to read generated schema entry: {e}"));
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.extension().and_then(|ext| ext.to_str()) == Some("json")
            && !expected.contains(file_name)
        {
            fs::remove_file(&path)
                .unwrap_or_else(|e| panic!("Failed to remove stale file {}: {e}", path.display()));
        }
    }
}

// ---------------------------------------------------------------------------
// MCP schema generator
// ---------------------------------------------------------------------------

fn generate_mcp_schema(tools: &[ToolContract]) -> (String, Vec<(String, String)>) {
    let mut include_lines = Vec::new();
    let mut schema_files = Vec::new();

    for tool in tools {
        let tool_name = &tool.name;
        let mut properties = serde_json::Map::new();
        let mut required: Vec<String> = Vec::new();

        for param in &tool.params {
            let mut prop = param.input_schema_object();

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
            "additionalProperties": false,
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

        let mut tool_entry = serde_json::json!({
            "name": tool_name,
            "description": tool.descriptions.mcp,
            "inputSchema": input_schema
        });
        // Per-tool outputSchema is optional. The typed registry parses it
        // up front so malformed schemas fail before artifacts render.
        if let Some(parsed) = &tool.output.schema {
            tool_entry
                .as_object_mut()
                .expect("tool entry is a JSON object")
                .insert("outputSchema".to_string(), parsed.clone());
        }
        let file_name = tool.artifacts.mcp_schema_file.clone();
        let json_str =
            serde_json::to_string_pretty(&tool_entry).expect("JSON serialization failed");
        include_lines.push(format!(
            "        serde_json::from_str(include_str!(\"generated_schema/{file_name}\"))\n            .expect(\"generated schema for {tool_name} is valid JSON\"),"
        ));
        schema_files.push((file_name, format!("{json_str}\n")));
    }

    let mut schema_rs = String::new();
    schema_rs.push_str("// AUTO-GENERATED by build.rs from tools.toml - do not edit\n");
    schema_rs.push_str("#![allow(clippy::all)]\n\n");
    schema_rs.push_str("pub fn generated_tool_list() -> serde_json::Value {\n");
    schema_rs.push_str("    let tools: Vec<serde_json::Value> = vec![\n");
    for line in include_lines {
        schema_rs.push_str(&line);
        schema_rs.push('\n');
    }
    schema_rs.push_str("    ];\n");
    schema_rs.push_str("    serde_json::json!({ \"tools\": tools })\n");
    schema_rs.push_str("}\n");

    (schema_rs, schema_files)
}

// ---------------------------------------------------------------------------
// CLI help constants generator
// ---------------------------------------------------------------------------

fn generate_cli_help(tools: &[ToolContract]) -> String {
    let mut lines = vec![
        "// AUTO-GENERATED by build.rs from tools.toml - do not edit".to_string(),
        "#![allow(clippy::all)]".to_string(),
    ];

    for tool in tools {
        let prefix = &tool.artifacts.cli_help_prefix;

        // Command-level about constant.
        let escaped = rust_escape(&tool.cli.about);
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
