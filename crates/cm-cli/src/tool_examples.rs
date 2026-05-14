//! Typed examples rendered into the public cx_* invocation syntax.

use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub struct ToolExample {
    pub invocation: String,
    pub arguments: Value,
}

impl ToolExample {
    fn from_args(tool_name: &str, args: Vec<ExampleArg>) -> Self {
        let invocation = render_invocation(tool_name, &args);
        let arguments = Value::Object(
            args.into_iter()
                .map(|arg| (arg.name.to_owned(), arg.value.into_value()))
                .collect::<Map<_, _>>(),
        );
        Self {
            invocation,
            arguments,
        }
    }
}

#[derive(Debug, Clone)]
struct ExampleArg {
    name: &'static str,
    value: ExampleValue,
}

#[derive(Debug, Clone)]
enum ExampleValue {
    String(&'static str),
    Array(Vec<ExampleValue>),
    Object(Vec<ExampleArg>),
}

impl ExampleValue {
    fn into_value(self) -> Value {
        match self {
            Self::String(value) => Value::String(value.to_owned()),
            Self::Array(values) => {
                Value::Array(values.into_iter().map(ExampleValue::into_value).collect())
            }
            Self::Object(args) => Value::Object(
                args.into_iter()
                    .map(|arg| (arg.name.to_owned(), arg.value.into_value()))
                    .collect::<Map<_, _>>(),
            ),
        }
    }

    fn render(&self) -> String {
        match self {
            Self::String(value) => serde_json::to_string(value).expect("string renders as JSON"),
            Self::Array(values) => {
                render_joined("[", "]", ", ", values.iter().map(ExampleValue::render))
            }
            Self::Object(args) => render_joined(
                "{",
                "}",
                ",",
                args.iter().map(|arg| {
                    let key = serde_json::to_string(arg.name).expect("key renders as JSON");
                    format!("{key}:{}", arg.value.render())
                }),
            ),
        }
    }
}

pub(crate) fn contract_examples(tool_name: &str) -> Result<Vec<ToolExample>, String> {
    let examples = match tool_name {
        "cx_recall" => vec![ToolExample::from_args(
            tool_name,
            vec![
                arg("query", string("auth decisions")),
                arg(
                    "scope",
                    object(vec![
                        arg("kind", string("path")),
                        arg("path", string("global/project:helioy")),
                    ]),
                ),
            ],
        )],
        "cx_search" => vec![ToolExample::from_args(
            tool_name,
            vec![
                arg("query", string("auth decisions")),
                arg("scope", object(vec![arg("kind", string("all"))])),
            ],
        )],
        "cx_store" => vec![ToolExample::from_args(
            tool_name,
            vec![
                arg("title", string("Use UUIDv7")),
                arg("body", string("...")),
                arg("kind", string("decision")),
            ],
        )],
        "cx_deposit" => vec![ToolExample::from_args(
            tool_name,
            vec![arg(
                "exchanges",
                array(vec![object(vec![
                    arg("user", string("...")),
                    arg("assistant", string("...")),
                ])]),
            )],
        )],
        "cx_browse" => vec![ToolExample::from_args(
            tool_name,
            vec![
                arg("kind", string("decision")),
                arg(
                    "scope",
                    object(vec![
                        arg("kind", string("path")),
                        arg("path", string("global/project:helioy")),
                    ]),
                ),
            ],
        )],
        "cx_get" => vec![ToolExample::from_args(
            tool_name,
            vec![arg("ids", array(vec![string("uuid1"), string("uuid2")]))],
        )],
        "cx_update" => vec![ToolExample::from_args(
            tool_name,
            vec![
                arg("id", string("uuid")),
                arg("title", string("Updated title")),
            ],
        )],
        "cx_forget" => vec![ToolExample::from_args(
            tool_name,
            vec![arg("ids", array(vec![string("uuid")]))],
        )],
        "cx_stats" => vec![ToolExample::from_args(tool_name, Vec::new())],
        "cx_export" => vec![ToolExample::from_args(
            tool_name,
            vec![arg("scope", string("global/project:helioy"))],
        )],
        _ => return Err(format!("missing contract examples for tool `{tool_name}`")),
    };

    Ok(examples)
}

fn arg(name: &'static str, value: ExampleValue) -> ExampleArg {
    ExampleArg { name, value }
}

fn string(value: &'static str) -> ExampleValue {
    ExampleValue::String(value)
}

fn array(values: Vec<ExampleValue>) -> ExampleValue {
    ExampleValue::Array(values)
}

fn object(args: Vec<ExampleArg>) -> ExampleValue {
    ExampleValue::Object(args)
}

fn render_invocation(tool_name: &str, args: &[ExampleArg]) -> String {
    render_joined(
        &format!("{tool_name}("),
        ")",
        ", ",
        args.iter()
            .map(|arg| format!("{}: {}", arg.name, arg.value.render())),
    )
}

fn render_joined<I>(prefix: &str, suffix: &str, separator: &str, parts: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let mut rendered = String::from(prefix);
    let mut first = true;
    for part in parts {
        if first {
            first = false;
        } else {
            rendered.push_str(separator);
        }
        rendered.push_str(&part);
    }
    rendered.push_str(suffix);
    rendered
}
