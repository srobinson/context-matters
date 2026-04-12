//! CLI command handlers for context-matters.

pub mod admin;
pub mod browse;
pub mod cli_def;
pub mod colors;
pub mod deposit;
pub mod errors;
pub mod forget;
pub mod get;
pub mod help_text;
pub mod recall;
pub mod scope;
pub mod stats;
pub mod store;
pub mod update;

#[path = "generated_help.rs"]
pub mod generated_help;

pub use admin::{cmd_init, cmd_serve, open_store};
pub use cli_def::{Cli, Commands};
