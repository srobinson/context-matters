//! Color-aware help strings for the `cm` CLI.
//!
//! All `color_print::cstr!` invocations in the crate live here. Other
//! files reference these `&'static str` constants instead of using the
//! macro directly, keeping the color-print boundary one file wide.

use color_print::cstr;

/// `clap` template that drops the auto-generated `{subcommands}` block —
/// our `before_help` already lists commands grouped READ / WRITE / ADMIN.
pub const HELP_TEMPLATE: &str = "{about-with-newline}\n{before-help}\n";

/// Short summary shown by `cm -h`.
pub const SHORT_HELP: &str = cstr!(
    r#"<bold><underline>READ</underline></bold>
  <bold>recall</bold>      Search one scope plus ancestors. Default: global.
  <bold>search</bold>      Content search across scopes. Requires --scope.
  <bold>browse</bold>      Filtered inventory with pagination. Default: cwd_inferred.
  <bold>get</bold>         Fetch full entry content by ID
  <bold>stats</bold>       Show store statistics and scope tree

<bold><underline>WRITE</underline></bold>
  <bold>store</bold>       Create a new entry (use the Curator UI)
  <bold>update</bold>      Partially update an entry
  <bold>deposit</bold>     Batch-store conversation exchanges
  <bold>forget</bold>      Soft-delete entries

<bold><underline>ADMIN</underline></bold>
  <bold>init</bold>        Write a default config file
  <bold>serve</bold>       Start the MCP server on stdio
  <bold>export</bold>      Export entries and scopes as JSON
  <bold>completions</bold> Generate shell completion script

Use <bold>--help</bold> for examples and the scope tip.
https://github.com/srobinson/context-matters"#
);

/// Long help shown by `cm --help`. Adds an examples block and the
/// scope-resolution hint.
pub const LONG_HELP: &str = cstr!(
    r#"<bold><underline>READ Commands</underline></bold>
  <bold>recall</bold>      Search one scope plus ancestors. Default: global.
  <bold>search</bold>      Content search across scopes. Requires --scope.
  <bold>browse</bold>      Filtered inventory with pagination. Default: cwd_inferred.
  <bold>get</bold>         Fetch full entry content by ID
  <bold>stats</bold>       Show store statistics and scope tree

<bold><underline>WRITE Commands</underline></bold>
  <bold>store</bold>       Create a new entry (use the Curator UI)
  <bold>update</bold>      Partially update an entry
  <bold>deposit</bold>     Batch-store conversation exchanges
  <bold>forget</bold>      Soft-delete entries

<bold><underline>ADMIN Commands</underline></bold>
  <bold>init</bold>        Write a default config file
  <bold>serve</bold>       Start the MCP server on stdio
  <bold>export</bold>      Export entries and scopes as JSON
  <bold>completions</bold> Generate shell completion script

<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm recall "auth migration"</bold>           <dim># tiered FTS5 with scope walk</dim>
  <dim>$</dim> <bold>cm search "auth migration" --scope '{"kind":"all"}'</bold>
  <dim>$</dim> <bold>cm browse --kind decision -j</bold>          <dim># JSON inventory of decisions</dim>
  <dim>$</dim> <bold>cm get 019d09ed-7a4f-7693</bold>             <dim># full entry by id</dim>
  <dim>$</dim> <bold>cm stats</bold>                              <dim># scope tree + counts</dim>
  <dim>$</dim> <bold>cm export --scope global/project:helioy</bold> <dim># JSON snapshot of a subtree</dim>

<bold><underline>Scope Resolution</underline></bold>
  Recall starts at <bold>global</bold>; search requires <bold>--scope</bold>;
  browse starts at <bold>cwd_inferred</bold>. Use <bold>--scope PATH</bold> for exact filtering.
  Run <bold>cm stats</bold> to discover all scope paths in the store.

https://github.com/srobinson/context-matters"#
);

/// `after_help` for `cm recall`.
pub const RECALL_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm recall</bold>                                          <dim># everything visible at global</dim>
  <dim>$</dim> <bold>cm recall "auth migration"</bold>                          <dim># FTS5 keyword search</dim>
  <dim>$</dim> <bold>cm recall --scope global/project:helioy --tags rust</bold>
  <dim>$</dim> <bold>cm recall --kinds decision,feedback --limit 5</bold>
  <dim>$</dim> <bold>cm recall "topic" -j | jq .entries</bold>                  <dim># JSON to jq</dim>"#
);

/// `after_help` for `cm search`.
pub const SEARCH_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm search "auth migration" --scope global/project:helioy</bold>
  <dim>$</dim> <bold>cm search "auth migration" --scope '{"kind":"all"}'</bold>
  <dim>$</dim> <bold>cm search "rust*" --scope '{"kind":"subtree","path":"global/project:helioy"}' -j</bold>
  <dim>$</dim> <bold>cm search "topic" --scope global --limit 5 --cursor eyJ...</bold>"#
);

/// `after_help` for `cm browse`.
pub const BROWSE_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm browse --kind decision</bold>                           <dim># inventory of decisions</dim>
  <dim>$</dim> <bold>cm browse --scope global/project:helioy --tag rust</bold>
  <dim>$</dim> <bold>cm browse --scope cwd_inferred --cwd /path/to/repo</bold>
  <dim>$</dim> <bold>cm browse --include-superseded --limit 50</bold>
  <dim>$</dim> <bold>cm browse -j</bold>                                        <dim># JSON for piping</dim>"#
);

/// `after_help` for `cm get`.
pub const GET_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm get 019d09ed-7a4f-7693-b9b3-bd152ed368a7</bold>
  <dim>$</dim> <bold>cm get id1 id2 id3</bold>                                  <dim># up to 100 ids</dim>
  <dim>$</dim> <bold>cm get id1 -j | jq .entries[0].body</bold>"#
);

/// `after_help` for `cm stats`.
pub const STATS_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm stats</bold>                                            <dim># human-readable</dim>
  <dim>$</dim> <bold>cm stats --tag-sort count</bold>                            <dim># tags by usage</dim>
  <dim>$</dim> <bold>cm stats -j</bold>                                          <dim># JSON for tooling</dim>"#
);

/// `after_help` for `cm store`.
pub const STORE_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Curator UI</underline></bold>
  Direct entry creation lives in the Curator web UI. Run:
  <dim>$</dim> <bold>cm serve --web</bold>                                       <dim># launch Curator UI</dim>
  Then open <bold>http://localhost:7878/curator</bold> in your browser."#
);

/// `after_help` for `cm update`.
pub const UPDATE_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm update 019d09ed --title "New title"</bold>
  <dim>$</dim> <bold>cm update 019d09ed --kind decision</bold>
  <dim>$</dim> <bold>cm update 019d09ed --meta '{"tags":["rust"],"priority":5}'</bold>"#
);

/// `after_help` for `cm deposit`.
pub const DEPOSIT_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm deposit --exchanges '[{"user":"...","assistant":"..."}]'</bold>
  <dim>$</dim> <bold>cm deposit --exchanges '[...]' --summary "session recap"</bold>
  <dim>$</dim> <bold>cm deposit --exchanges '[...]' --scope global/project:helioy</bold>
  <dim>$</dim> <bold>cat session.json | cm deposit --exchanges -</bold>         <dim># read blob from stdin</dim>"#
);

/// `after_help` for `cm forget`.
pub const FORGET_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm forget 019d09ed-7a4f-7693-b9b3-bd152ed368a7</bold>
  <dim>$</dim> <bold>cm forget id1 id2 id3</bold>                               <dim># up to 100 ids</dim>"#
);

/// `after_help` for `cm export`.
pub const EXPORT_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm export</bold>                                            <dim># full snapshot to stdout</dim>
  <dim>$</dim> <bold>cm export --scope global/project:helioy &gt; backup.json</bold>
  <dim>$</dim> <bold>cm export --format json</bold>                              <dim># default and currently the only format</dim>"#
);

/// `after_help` for `cm init`.
pub const INIT_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm init</bold>                                              <dim># write to ./.cm.config.toml</dim>
  <dim>$</dim> <bold>cm init --global</bold>                                     <dim># write to ~/.context-matters/</dim>
  <dim>$</dim> <bold>cm init --force</bold>                                      <dim># overwrite an existing file</dim>"#
);

/// `after_help` for `cm serve`.
pub const SERVE_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm serve</bold>                                             <dim># MCP server on stdio</dim>
  <dim>$</dim> <bold>cm serve --verbose</bold>                                   <dim># debug-level tracing on stderr</dim>"#
);

/// `after_help` for `cm completions`.
pub const COMPLETIONS_AFTER_HELP: &str = cstr!(
    r#"<bold><underline>Examples</underline></bold>
  <dim>$</dim> <bold>cm completions bash &gt; /etc/bash_completion.d/cm</bold>
  <dim>$</dim> <bold>cm completions zsh  &gt; ~/.zsh/_cm</bold>
  <dim>$</dim> <bold>cm completions fish &gt; ~/.config/fish/completions/cm.fish</bold>"#
);
