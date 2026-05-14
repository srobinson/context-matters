//! Typed cx tool contract registry.
//!
//! `tools.toml` remains the authored source. This module turns it into typed
//! Rust contracts that build-time generators and tests can share.

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::sync::OnceLock;

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

        Ok(Self {
            name,
            descriptions: ToolDescriptions {
                short: metadata.short_description.to_string(),
                mcp: raw.mcp_description,
            },
            params,
            required_params,
            scope_policy: metadata.scope_policy,
            examples: metadata
                .examples
                .iter()
                .map(|invocation| ToolExample {
                    invocation: invocation.to_string(),
                })
                .collect(),
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

    fn input_schema_object(self) -> Map<String, Value> {
        let one_of = self
            .fragments()
            .iter()
            .map(|fragment| fragment.schema_value())
            .collect();

        object_schema([("oneOf", Value::Array(one_of))])
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
    fn schema_value(self) -> Value {
        match self {
            Self::ExactPathString => serde_json::json!({
                "type": "string",
                "description": "Exact scope path string such as global/project:helioy/repo:context-matters, or cwd_inferred."
            }),
            Self::Path => scope_selector_schema(
                &["kind", "path"],
                object_schema([("kind", kind_schema(&["path"])), ("path", string_schema())]),
            ),
            Self::CwdInferred => scope_selector_schema(
                &["kind"],
                object_schema([
                    ("kind", kind_schema(&["cwd_inferred"])),
                    ("cwd", string_schema()),
                ]),
            ),
            Self::Project => scope_selector_schema(
                &["kind", "project"],
                object_schema([
                    ("kind", kind_schema(&["project"])),
                    ("project", string_schema()),
                ]),
            ),
            Self::Repo => scope_selector_schema(
                &["kind", "project", "repo"],
                object_schema([
                    ("kind", kind_schema(&["repo"])),
                    ("project", string_schema()),
                    ("repo", string_schema()),
                ]),
            ),
            Self::Session => scope_selector_schema(
                &["kind", "project", "session"],
                object_schema([
                    ("kind", kind_schema(&["session"])),
                    ("project", string_schema()),
                    ("repo", string_schema()),
                    ("session", string_schema()),
                ]),
            ),
            Self::Descendants => scope_selector_schema(
                &["kind", "path"],
                object_schema([
                    ("kind", kind_schema(&["descendants", "subtree"])),
                    ("path", string_schema()),
                ]),
            ),
            Self::Set => scope_selector_schema(
                &["kind", "paths"],
                object_schema([
                    ("kind", kind_schema(&["set"])),
                    (
                        "paths",
                        serde_json::json!({
                            "type": "array",
                            "items": {"type": "string"}
                        }),
                    ),
                ]),
            ),
            Self::All => {
                scope_selector_schema(&["kind"], object_schema([("kind", kind_schema(&["all"]))]))
            }
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
pub struct ToolExample {
    pub invocation: String,
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

fn scope_selector_schema(required: &[&str], properties: Map<String, Value>) -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

fn kind_schema(values: &[&str]) -> Value {
    serde_json::json!({
        "type": "string",
        "enum": values
    })
}

fn string_schema() -> Value {
    serde_json::json!({"type": "string"})
}

#[derive(Debug, Clone, Copy)]
struct ContractMetadata {
    short_description: &'static str,
    scope_policy: ScopePolicy,
    output_type: &'static str,
    examples: &'static [&'static str],
}

fn contract_metadata(tool_name: &str) -> Result<ContractMetadata, String> {
    let metadata = match tool_name {
        "cx_recall" => ContractMetadata {
            short_description: "Priority context for one known scope",
            scope_policy: ScopePolicy::SingularRead,
            output_type: "WebRecallView",
            examples: &[
                r#"cx_recall(query: "auth decisions", scope: {"kind":"path","path":"global/project:helioy"})"#,
            ],
        },
        "cx_search" => ContractMetadata {
            short_description: "Content search across wide or unknown scopes",
            scope_policy: ScopePolicy::BroadRead,
            output_type: "SearchView",
            examples: &[r#"cx_search(query: "auth decisions", scope: {"kind":"all"})"#],
        },
        "cx_store" => ContractMetadata {
            short_description: "Persist a fact, decision, preference, or lesson",
            scope_policy: ScopePolicy::SingularWrite,
            output_type: "StoreReceipt",
            examples: &[r#"cx_store(title: "Use UUIDv7", body: "...", kind: "decision")"#],
        },
        "cx_deposit" => ContractMetadata {
            short_description: "Batch-store conversation exchanges",
            scope_policy: ScopePolicy::SingularWrite,
            output_type: "DepositReceipt",
            examples: &[r#"cx_deposit(exchanges: [{user: "...", assistant: "..."}])"#],
        },
        "cx_browse" => ContractMetadata {
            short_description: "List entries with filters and pagination",
            scope_policy: ScopePolicy::BroadRead,
            output_type: "WebBrowseView",
            examples: &[
                r#"cx_browse(kind: "decision", scope: {"kind":"path","path":"global/project:helioy"})"#,
            ],
        },
        "cx_get" => ContractMetadata {
            short_description: "Fetch full content for specific entry IDs",
            scope_policy: ScopePolicy::NoScope,
            output_type: "WebGetView",
            examples: &[r#"cx_get(ids: ["uuid1", "uuid2"])"#],
        },
        "cx_update" => ContractMetadata {
            short_description: "Partially update an existing entry",
            scope_policy: ScopePolicy::SingularWrite,
            output_type: "WebUpdateView",
            examples: &[r#"cx_update(id: "uuid", title: "Updated title")"#],
        },
        "cx_forget" => ContractMetadata {
            short_description: "Mark entries forgotten so active reads skip them",
            scope_policy: ScopePolicy::NoScope,
            output_type: "ForgetReceipt",
            examples: &[r#"cx_forget(ids: ["uuid"])"#],
        },
        "cx_stats" => ContractMetadata {
            short_description: "View store statistics and scope breakdown",
            scope_policy: ScopePolicy::NoScope,
            output_type: "WebStatsView",
            examples: &["cx_stats()"],
        },
        "cx_export" => ContractMetadata {
            short_description: "Export entries as JSON for backup",
            scope_policy: ScopePolicy::BroadRead,
            output_type: "ExportView",
            examples: &[r#"cx_export(scope: "global/project:helioy")"#],
        },
        _ => return Err(format!("missing contract metadata for tool `{tool_name}`")),
    };

    Ok(metadata)
}
