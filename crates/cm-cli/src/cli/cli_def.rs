//! `clap` definition for the `cm` CLI.
//!
//! This file owns the entire `Cli` + `Commands` surface. Per-arg help text
//! comes from [`super::generated_help`] (single source of truth shared with
//! the MCP `tools/list` schema), and per-command `after_help` blocks come
//! from [`super::help_text`]. No `///` doc comments are used for clap help
//! — `#[arg(help = …)]` is used everywhere so the help strings stay in
//! lockstep with the generated table.

use std::path::PathBuf;

use clap::{ColorChoice, Parser, Subcommand};
use clap_complete::Shell;

use super::generated_help as gh;
use super::help_text as ht;

/// Top-level `cm` parser.
#[derive(Parser, Debug)]
#[command(
    name = "cm",
    about = "Structured context store for AI agents",
    long_about = "Structured context store for AI agents",
    before_help = ht::SHORT_HELP,
    before_long_help = ht::LONG_HELP,
    help_template = ht::HELP_TEMPLATE,
    version = crate::VERSION,
    color = ColorChoice::Auto,
    subcommand_required = false,
    disable_help_subcommand = true,
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        help = "Enable verbose debug output (debug-level tracing on stderr)"
    )]
    pub verbose: bool,

    /// Hidden: emit clap-derived markdown reference to stdout. Used to
    /// regenerate the CLI section of `README.md`. Mutually exclusive with
    /// any subcommand.
    #[arg(long, hide = true, exclusive = true)]
    pub markdown_help: bool,

    /// Hidden: emit one roff `.1` file per subcommand into `<DIR>`. Used by
    /// the release pipeline to ship man pages. Mutually exclusive with any
    /// subcommand.
    #[arg(long, hide = true, value_name = "DIR", exclusive = true)]
    pub generate_man_pages: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Thirteen-variant `cm` subcommand surface. The cluster ordering matches
/// the READ / WRITE / ADMIN groups in [`super::help_text::SHORT_HELP`].
#[derive(Subcommand, Debug)]
pub enum Commands {
    // ---------------- READ ----------------
    /// Search and retrieve context entries from the store.
    #[command(long_about = gh::RECALL_ABOUT, after_help = ht::RECALL_AFTER_HELP)]
    Recall {
        #[arg(help = gh::RECALL_QUERY_HELP)]
        query: Option<String>,
        #[arg(long, help = gh::RECALL_SCOPE_HELP)]
        scope: Option<String>,
        #[arg(long, value_delimiter = ',', help = gh::RECALL_KINDS_HELP)]
        kinds: Vec<String>,
        #[arg(long, value_delimiter = ',', help = gh::RECALL_TAGS_HELP)]
        tags: Vec<String>,
        #[arg(long, help = gh::RECALL_LIMIT_HELP)]
        limit: Option<u32>,
        #[arg(long, help = gh::RECALL_MAX_TOKENS_HELP)]
        max_tokens: Option<u32>,
        #[arg(short = 'j', long, help = "Emit JSON instead of human-readable text")]
        json: bool,
    },

    /// Search entries by content across explicit scopes.
    #[command(long_about = gh::SEARCH_ABOUT, after_help = ht::SEARCH_AFTER_HELP)]
    Search {
        #[arg(help = gh::SEARCH_QUERY_HELP)]
        query: String,
        #[arg(long, required = true, help = gh::SEARCH_SCOPE_HELP)]
        scope: String,
        #[arg(long, value_delimiter = ',', help = gh::SEARCH_KINDS_HELP)]
        kinds: Vec<String>,
        #[arg(long, value_delimiter = ',', help = gh::SEARCH_TAGS_HELP)]
        tags: Vec<String>,
        #[arg(long, help = gh::SEARCH_LIMIT_HELP)]
        limit: Option<u32>,
        #[arg(long, help = gh::SEARCH_CURSOR_HELP)]
        cursor: Option<String>,
        #[arg(short = 'j', long, help = "Emit JSON instead of human-readable text")]
        json: bool,
    },

    /// Browse entries with filtering and pagination.
    #[command(long_about = gh::BROWSE_ABOUT, after_help = ht::BROWSE_AFTER_HELP)]
    Browse {
        #[arg(long, help = gh::BROWSE_SCOPE_HELP)]
        scope: Option<String>,
        #[arg(
            long,
            help = "Working directory used for cwd_inferred scope resolution"
        )]
        cwd: Option<String>,
        #[arg(long, help = gh::BROWSE_INCLUDE_RESOLUTION_HELP)]
        include_resolution: bool,
        #[arg(long, help = gh::BROWSE_KIND_HELP)]
        kind: Option<String>,
        #[arg(long, help = gh::BROWSE_TAG_HELP)]
        tag: Option<String>,
        #[arg(long, help = gh::BROWSE_CREATED_BY_HELP)]
        created_by: Option<String>,
        #[arg(long, help = gh::BROWSE_INCLUDE_SUPERSEDED_HELP)]
        include_superseded: bool,
        #[arg(long, help = gh::BROWSE_LIMIT_HELP)]
        limit: Option<u32>,
        #[arg(long, help = gh::BROWSE_CURSOR_HELP)]
        cursor: Option<String>,
        #[arg(short = 'j', long, help = "Emit JSON instead of human-readable text")]
        json: bool,
    },

    /// Fetch full entry content by ID.
    #[command(long_about = gh::GET_ABOUT, after_help = ht::GET_AFTER_HELP)]
    Get {
        #[arg(help = gh::GET_IDS_HELP)]
        ids: Vec<String>,
        #[arg(short = 'j', long, help = "Emit JSON instead of human-readable text")]
        json: bool,
    },

    /// Show store statistics.
    #[command(long_about = gh::STATS_ABOUT, after_help = ht::STATS_AFTER_HELP)]
    Stats {
        #[arg(long, help = gh::STATS_TAG_SORT_HELP)]
        tag_sort: Option<String>,
        #[arg(short = 'j', long, help = "Emit JSON instead of human-readable text")]
        json: bool,
    },

    // ---------------- WRITE ----------------
    /// Store a new entry. The CLI surface mirrors the MCP `cx_store` tool;
    /// the canonical interactive entry path is the Curator UI under
    /// `cm serve --web`.
    #[command(long_about = gh::STORE_ABOUT, after_help = ht::STORE_AFTER_HELP)]
    Store {
        #[arg(long, help = gh::STORE_TITLE_HELP)]
        title: Option<String>,
        #[arg(long, help = gh::STORE_BODY_HELP)]
        body: Option<String>,
        #[arg(long, help = gh::STORE_KIND_HELP)]
        kind: Option<String>,
        #[arg(long, help = gh::STORE_SCOPE_HELP)]
        scope: Option<String>,
        #[arg(long, help = gh::STORE_CREATED_BY_HELP)]
        created_by: Option<String>,
        #[arg(long, value_delimiter = ',', help = gh::STORE_TAGS_HELP)]
        tags: Vec<String>,
        #[arg(long, help = gh::STORE_CONFIDENCE_HELP)]
        confidence: Option<String>,
        #[arg(long, help = gh::STORE_SOURCE_HELP)]
        source: Option<String>,
        #[arg(long, help = gh::STORE_EXPIRES_AT_HELP)]
        expires_at: Option<String>,
        #[arg(long, help = gh::STORE_PRIORITY_HELP)]
        priority: Option<i32>,
        #[arg(long, help = gh::STORE_SUPERSEDES_HELP)]
        supersedes: Option<String>,
    },

    /// Partially update an entry.
    #[command(long_about = gh::UPDATE_ABOUT, after_help = ht::UPDATE_AFTER_HELP)]
    Update {
        #[arg(help = gh::UPDATE_ID_HELP)]
        id: String,
        #[arg(long, help = gh::UPDATE_TITLE_HELP)]
        title: Option<String>,
        #[arg(long, help = gh::UPDATE_BODY_HELP)]
        body: Option<String>,
        #[arg(long, help = gh::UPDATE_KIND_HELP)]
        kind: Option<String>,
        #[arg(long, help = gh::UPDATE_META_HELP)]
        meta: Option<String>,
        #[arg(short = 'j', long, help = "Emit JSON instead of human-readable text")]
        json: bool,
    },

    /// Batch-store conversation exchanges.
    #[command(long_about = gh::DEPOSIT_ABOUT, after_help = ht::DEPOSIT_AFTER_HELP)]
    Deposit {
        #[arg(long, help = gh::DEPOSIT_EXCHANGES_HELP)]
        exchanges: String,
        #[arg(long, help = gh::DEPOSIT_SUMMARY_HELP)]
        summary: Option<String>,
        #[arg(long, help = gh::DEPOSIT_SCOPE_HELP)]
        scope: Option<String>,
        #[arg(long, help = gh::DEPOSIT_CREATED_BY_HELP)]
        created_by: Option<String>,
        #[arg(short = 'j', long, help = "Emit JSON instead of human-readable text")]
        json: bool,
    },

    /// Soft-delete entries.
    #[command(long_about = gh::FORGET_ABOUT, after_help = ht::FORGET_AFTER_HELP)]
    Forget {
        #[arg(required = true, num_args = 1..=100, help = gh::FORGET_IDS_HELP)]
        ids: Vec<String>,
    },

    // ---------------- ADMIN ----------------
    /// Generate a commented config file with default values.
    #[command(after_help = ht::INIT_AFTER_HELP)]
    Init {
        #[arg(long, help = "Write to ~/.context-matters/ instead of CWD")]
        global: bool,
        #[arg(long, help = "Overwrite an existing config file")]
        force: bool,
    },

    /// Start the MCP server on stdio transport.
    #[command(after_help = ht::SERVE_AFTER_HELP)]
    Serve,

    /// Export entries and scopes as JSON.
    #[command(long_about = gh::EXPORT_ABOUT, after_help = ht::EXPORT_AFTER_HELP)]
    Export {
        #[arg(long, help = gh::EXPORT_SCOPE_HELP)]
        scope: Option<String>,
        #[arg(long, help = gh::EXPORT_FORMAT_HELP)]
        format: Option<String>,
    },

    /// Generate a shell completion script and write it to stdout.
    #[command(after_help = ht::COMPLETIONS_AFTER_HELP)]
    Completions {
        #[arg(value_enum, help = "Target shell: bash, zsh, fish, powershell, elvish")]
        shell: Shell,
    },
}
