//! Typed cx tool contract registry.
//!
//! `tools.toml` remains the authored source. This module turns it into typed
//! Rust contracts that build-time generators and tests can share.

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::sync::OnceLock;

pub use crate::tool_examples::ToolExample;
use crate::tool_examples::contract_examples;

const TOOLS_TOML: &str = include_str!("../../../tools.toml");

static REGISTRY: OnceLock<ToolContractRegistry> = OnceLock::new();

pub fn contract_registry() -> &'static ToolContractRegistry {
    REGISTRY.get_or_init(|| {
        ToolContractRegistry::from_toml_str(TOOLS_TOML)
            .unwrap_or_else(|e| panic!("tools.toml contract registry is valid: {e}"))
    })
}

#[derive(Debug, Clone)]
pub struct ToolContractRegistry {
    skill: Option<SkillConfig>,
    tools: Vec<ToolContract>,
}

impl ToolContractRegistry {
    pub fn from_toml_str(content: &str) -> Result<Self, String> {
        let parsed: RawToolsToml =
            toml::from_str(content).map_err(|e| format!("failed to parse tools.toml: {e}"))?;

        let tools = parsed
            .tools
            .into_iter()
            .map(|(name, raw)| ToolContract::from_raw(name, raw))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            skill: parsed.skill,
            tools,
        })
    }

    pub fn skill(&self) -> Option<&SkillConfig> {
        self.skill.as_ref()
    }

    pub fn tools(&self) -> &[ToolContract] {
        &self.tools
    }

    pub fn get(&self, name: &str) -> Option<&ToolContract> {
        self.tools.iter().find(|tool| tool.name == name)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillConfig {
    pub workflow: String,
}

#[derive(Debug, Clone)]
pub struct ToolContract {
    pub name: String,
    pub descriptions: ToolDescriptions,
    pub params: Vec<ToolParamContract>,
    pub required_params: Vec<String>,
    pub scope_policy: ScopePolicy,
    pub examples: Vec<ToolExample>,
    pub output: OutputContract,
    pub cli: CliMetadata,
    pub artifacts: ArtifactRenderMetadata,
}

impl ToolContract {
    fn from_raw(name: String, raw: RawToolDef) -> Result<Self, String> {
        let metadata = contract_metadata(&name)?;
        let params = raw
            .params
            .into_iter()
            .map(|param| ToolParamContract::from_raw(&name, param))
            .collect::<Result<Vec<_>, _>>()?;
        let required_params = params
            .iter()
            .filter(|param| param.required)
            .map(|param| param.name.clone())
            .collect();
        let output = OutputContract::from_raw(&name, metadata.output_type, raw.output_schema)?;
        let artifacts = ArtifactRenderMetadata::for_tool(&name, &raw.cli_name);
        let examples = contract_examples(&name)?;

        Ok(Self {
            name,
            descriptions: ToolDescriptions {
                short: metadata.short_description.to_string(),
                mcp: raw.mcp_description,
            },
            params,
            required_params,
            scope_policy: metadata.scope_policy,
            examples,
            output,
            cli: CliMetadata {
                name: raw.cli_name,
                about: raw.cli_about,
            },
            artifacts,
        })
    }

    pub fn param(&self, name: &str) -> Option<&ToolParamContract> {
        self.params.iter().find(|param| param.name == name)
    }

    pub fn required_param_names(&self) -> Vec<&str> {
        self.required_params.iter().map(String::as_str).collect()
    }
}

#[derive(Debug, Clone)]
pub struct ToolDescriptions {
    pub short: String,
    pub mcp: String,
}

#[derive(Debug, Clone)]
pub struct ToolParamContract {
    pub name: String,
    pub shape: ParamShape,
    pub required: bool,
    pub enum_values: Option<Vec<String>>,
    pub mcp_description: String,
    pub cli_help: Option<String>,
    pub cli_flag: Option<String>,
}

impl ToolParamContract {
    fn from_raw(tool_name: &str, raw: RawParamDef) -> Result<Self, String> {
        let shape = ParamShape::from_raw(tool_name, &raw)?;

        Ok(Self {
            name: raw.name,
            shape,
            required: raw.required,
            enum_values: raw.enum_values,
            mcp_description: raw.mcp_description,
            cli_help: raw.cli_help,
            cli_flag: raw.cli_flag,
        })
    }

    pub fn input_schema_object(&self) -> Map<String, Value> {
        self.shape.input_schema_object()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamShape {
    Scalar(String),
    Array { items: ArrayItems },
    Scope(ScopeInputShape),
    CustomSchema(Value),
}

impl ParamShape {
    fn from_raw(tool_name: &str, raw: &RawParamDef) -> Result<Self, String> {
        if let Some(scope_shape) = raw.scope_schema {
            if raw.name != "scope" {
                return Err(format!(
                    "scope_schema is only valid for the `scope` param on tool `{tool_name}`"
                ));
            }
            if raw.mcp_schema.is_some() {
                return Err(format!(
                    "param `scope` on tool `{tool_name}` must use scope_schema or mcp_schema, not both"
                ));
            }
            return Ok(Self::Scope(scope_shape));
        }

        if let Some(raw_schema) = &raw.mcp_schema {
            let schema: Value = serde_json::from_str(raw_schema).map_err(|e| {
                format!(
                    "mcp_schema for param `{}` on tool `{tool_name}` is not valid JSON: {e}",
                    raw.name
                )
            })?;
            if !schema.is_object() {
                return Err(format!(
                    "mcp_schema for param `{}` on tool `{tool_name}` must be a JSON object",
                    raw.name
                ));
            }
            return Ok(Self::CustomSchema(schema));
        }

        if raw.type_ == "array" {
            return Ok(Self::Array {
                items: match &raw.items_type {
                    Some(scalar) => ArrayItems::Scalar(scalar.clone()),
                    None => ArrayItems::ExchangeObject,
                },
            });
        }

        Ok(Self::Scalar(raw.type_.clone()))
    }

    fn input_schema_object(&self) -> Map<String, Value> {
        match self {
            Self::Scalar(kind) => object_schema([("type", Value::String(kind.clone()))]),
            Self::Array { items } => object_schema([
                ("type", Value::String("array".to_string())),
                ("items", items.schema_value()),
            ]),
            Self::Scope(scope) => scope.input_schema_object(),
            Self::CustomSchema(schema) => schema
                .as_object()
                .unwrap_or_else(|| panic!("custom parameter schema is a JSON object"))
                .clone(),
        }
    }
}

const SINGULAR_SCOPE_FRAGMENTS: [ScopeFragment; 6] = [
    ScopeFragment::ExactPathString,
    ScopeFragment::Path,
    ScopeFragment::CwdInferred,
    ScopeFragment::Project,
    ScopeFragment::Repo,
    ScopeFragment::Session,
];

const BROAD_SCOPE_FRAGMENTS: [ScopeFragment; 9] = [
    ScopeFragment::ExactPathString,
    ScopeFragment::Path,
    ScopeFragment::CwdInferred,
    ScopeFragment::Project,
    ScopeFragment::Repo,
    ScopeFragment::Session,
    ScopeFragment::Descendants,
    ScopeFragment::Set,
    ScopeFragment::All,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeInputShape {
    Singular,
    Broad,
}

impl ScopeInputShape {
    pub fn fragments(self) -> &'static [ScopeFragment] {
        match self {
            Self::Singular => &SINGULAR_SCOPE_FRAGMENTS,
            Self::Broad => &BROAD_SCOPE_FRAGMENTS,
        }
    }

    /// Emit a Codex-safe flat-object schema for the scope parameter.
    ///
    /// The earlier shape was a top-level `oneOf` of per-kind object
    /// variants plus a bare string. OpenAI Codex's strict-mode tool
    /// validator (and Gemini's pipeline) rejects top-level
    /// `oneOf`/`anyOf`/`allOf`/`not` on any parameter before the model
    /// is invoked, returning HTTP 400 — the model never sees the prose
    /// or the validation errors. This emitter produces one flat object:
    ///   `{ kind: <enum>, path?, paths?, project?, repo?, session?, cwd? }`
    /// with `additionalProperties: false` and per-kind required-field
    /// validation enforced server-side in
    /// `cm_capabilities::scope::types::ScopeSelectorWireIn::into_selector`.
    /// See ALP-2476 and openai/codex#2204.
    fn input_schema_object(self) -> Map<String, Value> {
        let kinds: Vec<&'static str> = self
            .fragments()
            .iter()
            .filter_map(|fragment| fragment.canonical_kind())
            .collect();

        let mut properties = Map::new();
        properties.insert("kind".to_string(), kind_enum_schema(&kinds));
        properties.insert("path".to_string(), property_schema(
            "Canonical scope path beginning with 'global'. Required for kind 'path'; required for kind 'subtree' on broad shapes.",
        ));
        properties.insert(
            "cwd".to_string(),
            property_schema(
                "Filesystem path used for scope inference. Optional for kind 'cwd_inferred'.",
            ),
        );
        properties.insert(
            "project".to_string(),
            property_schema(
                "Project identifier. Required for kind 'project', 'repo', and 'session'.",
            ),
        );
        properties.insert(
            "repo".to_string(),
            property_schema(
                "Repo identifier. Required for kind 'repo'. Optional for kind 'session'.",
            ),
        );
        properties.insert(
            "session".to_string(),
            property_schema("Session identifier. Required for kind 'session'."),
        );
        if matches!(self, Self::Broad) {
            properties.insert(
                "paths".to_string(),
                serde_json::json!({
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of canonical scope paths. Required for kind 'set'."
                }),
            );
        }

        let mut schema = Map::new();
        schema.insert("type".to_string(), Value::String("object".to_string()));
        schema.insert("additionalProperties".to_string(), Value::Bool(false));
        schema.insert(
            "required".to_string(),
            Value::Array(vec![Value::String("kind".to_string())]),
        );
        schema.insert("properties".to_string(), Value::Object(properties));
        schema.insert(
            "examples".to_string(),
            Value::Array(self.canonical_examples()),
        );
        schema
    }

    fn canonical_examples(self) -> Vec<Value> {
        match self {
            Self::Singular => vec![
                serde_json::json!({"kind": "path", "path": "global/project:helioy"}),
                serde_json::json!({"kind": "cwd_inferred"}),
                serde_json::json!({
                    "kind": "repo",
                    "project": "helioy",
                    "repo": "context-matters"
                }),
            ],
            Self::Broad => vec![
                serde_json::json!({"kind": "all"}),
                serde_json::json!({
                    "kind": "subtree",
                    "path": "global/project:helioy"
                }),
                serde_json::json!({
                    "kind": "path",
                    "path": "global/project:helioy/repo:context-matters"
                }),
                serde_json::json!({
                    "kind": "set",
                    "paths": [
                        "global/project:helioy",
                        "global/project:helioy/repo:context-matters"
                    ]
                }),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeFragment {
    ExactPathString,
    Path,
    CwdInferred,
    Project,
    Repo,
    Session,
    Descendants,
    Set,
    All,
}

impl ScopeFragment {
    pub fn request_example(self) -> &'static str {
        match self {
            Self::ExactPathString => r#"{ "scope": "global/project:helioy/repo:context-matters" }"#,
            Self::Path => {
                r#"{ "scope": { "kind": "path", "path": "global/project:helioy/repo:context-matters" } }"#
            }
            Self::CwdInferred => {
                r#"{ "scope": { "kind": "cwd_inferred", "cwd": "/path/to/repo" } }"#
            }
            Self::Project => r#"{ "scope": { "kind": "project", "project": "helioy" } }"#,
            Self::Repo => {
                r#"{ "scope": { "kind": "repo", "project": "helioy", "repo": "context-matters" } }"#
            }
            Self::Session => {
                r#"{ "scope": { "kind": "session", "project": "helioy", "repo": "context-matters", "session": "abc" } }"#
            }
            Self::Descendants => {
                r#"{ "scope": { "kind": "descendants", "path": "global/project:helioy" } }"#
            }
            Self::Set => {
                r#"{ "scope": { "kind": "set", "paths": ["global", "global/project:helioy"] } }"#
            }
            Self::All => r#"{ "scope": { "kind": "all" } }"#,
        }
    }

    /// Canonical `kind` discriminator string for this fragment.
    ///
    /// Returns `None` for `ExactPathString` because the bare-string
    /// shape is no longer advertised in the generated schema (top-level
    /// `string`-or-`object` `oneOf` is the pattern OpenAI Codex's
    /// strict-mode validator rejects). The parser still accepts bare
    /// scope path strings for CLI ergonomics; see
    /// `cm_capabilities::scope::types::ScopeSelector::parse`.
    ///
    /// `Descendants` collapses to the canonical `"subtree"`. The parser
    /// continues to accept `"descendants"` via a serde alias.
    pub fn canonical_kind(self) -> Option<&'static str> {
        match self {
            Self::ExactPathString => None,
            Self::Path => Some("path"),
            Self::CwdInferred => Some("cwd_inferred"),
            Self::Project => Some("project"),
            Self::Repo => Some("repo"),
            Self::Session => Some("session"),
            Self::Descendants => Some("subtree"),
            Self::Set => Some("set"),
            Self::All => Some("all"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayItems {
    Scalar(String),
    ExchangeObject,
}

impl ArrayItems {
    fn schema_value(&self) -> Value {
        match self {
            Self::Scalar(kind) => serde_json::json!({"type": kind}),
            Self::ExchangeObject => serde_json::json!({
                "type": "object",
                "properties": {
                    "user": {"type": "string"},
                    "assistant": {"type": "string"}
                },
                "required": ["user", "assistant"]
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopePolicy {
    NoScope,
    SingularRead,
    BroadRead,
    SingularWrite,
}

#[derive(Debug, Clone)]
pub struct OutputContract {
    pub type_name: String,
    pub schema: Option<Value>,
}

impl OutputContract {
    fn from_raw(
        tool_name: &str,
        type_name: &'static str,
        schema: Option<String>,
    ) -> Result<Self, String> {
        let schema = schema
            .map(|raw| {
                serde_json::from_str(&raw)
                    .map_err(|e| format!("output_schema for tool `{tool_name}` is invalid: {e}"))
            })
            .transpose()?;

        Ok(Self {
            type_name: type_name.to_string(),
            schema,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CliMetadata {
    pub name: String,
    pub about: String,
}

#[derive(Debug, Clone)]
pub struct ArtifactRenderMetadata {
    pub mcp_schema_file: String,
    pub cli_help_prefix: String,
}

impl ArtifactRenderMetadata {
    fn for_tool(tool_name: &str, cli_name: &str) -> Self {
        Self {
            mcp_schema_file: format!("{}.json", tool_name.replace('-', "_")),
            cli_help_prefix: cli_name.to_uppercase().replace('-', "_"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawToolsToml {
    skill: Option<SkillConfig>,
    tools: IndexMap<String, RawToolDef>,
}

#[derive(Debug, Deserialize)]
struct RawToolDef {
    cli_name: String,
    mcp_description: String,
    cli_about: String,
    output_schema: Option<String>,
    #[serde(default)]
    params: Vec<RawParamDef>,
}

#[derive(Debug, Deserialize)]
struct RawParamDef {
    name: String,
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    required: bool,
    #[serde(rename = "enum")]
    enum_values: Option<Vec<String>>,
    mcp_description: String,
    scope_schema: Option<ScopeInputShape>,
    mcp_schema: Option<String>,
    cli_help: Option<String>,
    cli_flag: Option<String>,
    items_type: Option<String>,
}

fn object_schema(items: impl IntoIterator<Item = (&'static str, Value)>) -> Map<String, Value> {
    items
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn kind_enum_schema(values: &[&str]) -> Value {
    serde_json::json!({
        "type": "string",
        "enum": values,
        "description": "Discriminator for the scope variant."
    })
}

fn property_schema(description: &str) -> Value {
    serde_json::json!({
        "type": "string",
        "description": description,
    })
}

#[derive(Debug, Clone, Copy)]
struct ContractMetadata {
    short_description: &'static str,
    scope_policy: ScopePolicy,
    output_type: &'static str,
}

fn contract_metadata(tool_name: &str) -> Result<ContractMetadata, String> {
    let metadata = match tool_name {
        "cx_recall" => ContractMetadata {
            short_description: "Priority context for one known scope",
            scope_policy: ScopePolicy::SingularRead,
            output_type: "WebRecallView",
        },
        "cx_search" => ContractMetadata {
            short_description: "Content search across wide or unknown scopes",
            scope_policy: ScopePolicy::BroadRead,
            output_type: "SearchView",
        },
        "cx_store" => ContractMetadata {
            short_description: "Persist a fact, decision, preference, or lesson",
            scope_policy: ScopePolicy::SingularWrite,
            output_type: "StoreReceipt",
        },
        "cx_deposit" => ContractMetadata {
            short_description: "Batch-store conversation exchanges",
            scope_policy: ScopePolicy::SingularWrite,
            output_type: "DepositReceipt",
        },
        "cx_browse" => ContractMetadata {
            short_description: "List entries with filters and pagination",
            scope_policy: ScopePolicy::BroadRead,
            output_type: "WebBrowseView",
        },
        "cx_get" => ContractMetadata {
            short_description: "Fetch full content for specific entry IDs",
            scope_policy: ScopePolicy::NoScope,
            output_type: "WebGetView",
        },
        "cx_update" => ContractMetadata {
            short_description: "Partially update an existing entry",
            scope_policy: ScopePolicy::SingularWrite,
            output_type: "UpdateReceipt",
        },
        "cx_forget" => ContractMetadata {
            short_description: "Mark entries forgotten so active reads skip them",
            scope_policy: ScopePolicy::NoScope,
            output_type: "ForgetReceipt",
        },
        "cx_stats" => ContractMetadata {
            short_description: "View store statistics and scope breakdown",
            scope_policy: ScopePolicy::NoScope,
            output_type: "WebStatsView",
        },
        "cx_export" => ContractMetadata {
            short_description: "Export entries as JSON for backup",
            scope_policy: ScopePolicy::BroadRead,
            output_type: "ExportView",
        },
        _ => return Err(format!("missing contract metadata for tool `{tool_name}`")),
    };

    Ok(metadata)
}
