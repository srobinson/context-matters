# context-matters

Structured context store for AI agents. SQLite-backed, scoped, searchable.

Agents accumulate knowledge during sessions: decisions made, feedback received, patterns discovered, lessons learned. Without persistence, every session starts from zero. context-matters gives agents a place to store and retrieve that knowledge across sessions, projects, and scopes.

## Install

```bash
npx context-matters serve    # MCP server on stdio
```

Or build from source:

```bash
cargo install --path crates/cm-cli
```

## MCP server

Runs as a Model Context Protocol server. Ten tools, all prefixed `cx_*`.

```bash
cm serve
```

This CLI mirrors the MCP tool surface. From a shell, use `cm <command>`.
From an MCP client, the same operations are exposed as `cx_<command>`.
Run `cm serve` to start the MCP server on stdio.

| Tool | Purpose |
|------|---------|
| `cx_recall` | Priority context for one known scope via ancestor walk |
| `cx_search` | Content search across wide or unknown scopes |
| `cx_store` | Persist a fact, decision, preference, or lesson |
| `cx_deposit` | Batch-store conversation exchanges for continuity |
| `cx_browse` | List entries with filters and cursor pagination |
| `cx_get` | Fetch full content for specific entry IDs |
| `cx_update` | Partially update an existing entry |
| `cx_forget` | Mark entries forgotten so active reads skip them |
| `cx_stats` | Store statistics and scope breakdown |
| `cx_export` | Export entries as JSON for backup |

## CLI read commands

The CLI reads from the same store as the `cx_*` MCP tools.

| Command | Scope contract |
|---------|----------------|
| `cm recall` | Search one scope plus ancestors. Default: `global`. |
| `cm search` | Content search across scopes. Requires `--scope`. |
| `cm browse` | Filtered inventory with pagination. Default: `cwd_inferred`. |

## Web UI

Run `cm-web --open` for browser entry management and monitoring. It serves `http://localhost:3141/` by default.

## Scope model

Context is hierarchical. Broader scopes are visible at narrower scopes.

```
global                                        cross-project knowledge
global/project:helioy                         project-level decisions
global/project:helioy/repo:fmm               codebase-specific facts
global/project:helioy/repo:fmm/session:abc   ephemeral task context
```

Public request inputs select scope with structured `scope` JSON:

```json
{ "scope": { "kind": "path", "path": "global/project:helioy/repo:fmm" } }
```

```json
{ "scope": { "kind": "cwd_inferred", "cwd": "/path/to/repo" } }
```

Other read selectors include `subtree`, `set`, and `all`. `cx_recall` accepts only `path` and `cwd_inferred`; use `cx_search` for broad content search. `cwd_inferred` uses git metadata when available, so linked worktrees resolve to the source repository identity instead of the transient worktree directory.

Persisted entries, export rows, response payloads, and internal exact path types include a `scope_path` field that identifies the exact stored scope of each row.

## Architecture

Five crates, clean separation:

| Crate | What it does |
|-------|-------------|
| `cm-core` | Domain types, ContextStore trait, query construction. Zero I/O. |
| `cm-store` | SQLite adapter via sqlx. WAL mode, FTS5 search, BLAKE3 dedup. |
| `cm-capabilities` | Shared request/response types, validation, scope resolution, projections. |
| `cm-cli` | CLI binary + MCP server. Tool docs generated from `tools.toml`. |
| `cm-web` | Web monitoring dashboard with Axum and React/Vite. Run `cm-web --open`; default URL: `http://localhost:3141/`. |

## Development

Rust 2024 edition. [just](https://github.com/casey/just) as task runner.

```bash
just check    # fmt + clippy (warnings = errors)
just build    # cargo build --workspace
just test     # 161 tests
just fmt      # rustfmt
```

## License

MIT
