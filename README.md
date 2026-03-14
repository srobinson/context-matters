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

Runs as a Model Context Protocol server. Nine tools, all prefixed `cx_*`.

```bash
cm serve
```

| Tool | Purpose |
|------|---------|
| `cx_recall` | Search and retrieve context relevant to the current task |
| `cx_store` | Persist a fact, decision, preference, or lesson |
| `cx_deposit` | Batch-store conversation exchanges for continuity |
| `cx_browse` | List entries with filters and cursor pagination |
| `cx_get` | Fetch full content for specific entry IDs |
| `cx_update` | Partially update an existing entry |
| `cx_forget` | Soft-delete entries no longer relevant |
| `cx_stats` | Store statistics and scope breakdown |
| `cx_export` | Export entries as JSON for backup |

## Scope model

Context is hierarchical. Broader scopes are visible at narrower scopes.

```
global                                        cross-project knowledge
global/project:helioy                         project-level decisions
global/project:helioy/repo:fmm               codebase-specific facts
global/project:helioy/repo:fmm/session:abc   ephemeral task context
```

## Architecture

Three crates, clean separation:

| Crate | What it does |
|-------|-------------|
| `cm-core` | Domain types, ContextStore trait, query construction. Zero I/O. |
| `cm-store` | SQLite adapter via sqlx. WAL mode, FTS5 search, BLAKE3 dedup. |
| `cm-cli` | CLI binary + MCP server. Tool docs generated from `tools.toml`. |

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
