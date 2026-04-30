# context-matters

Structured context store for AI agents. Part of the [helioy](https://github.com/srobinson/context-matters) ecosystem.

## What it is

A Rust MCP server that gives AI agents persistent memory across sessions. Agents store facts, decisions, preferences, lessons, and feedback into a SQLite database, then retrieve them through scoped recall or broad content search.

## Why it exists

AI agents lose all context between sessions. context-matters solves this by providing a structured, queryable store that agents interact with through 10 MCP tools (prefixed `cx_`). Feedback from user corrections gets highest recall priority, so agents learn from mistakes.

## How it works

**Storage**: SQLite with FTS5, WAL mode, BLAKE3 content hashing for deduplication, UUID v7 for time-sortable IDs.

**Scope hierarchy**: `global > project > repo > session`. Recall at a narrow scope automatically walks up to ancestors, so project-level decisions are visible from any repo within that project.

**Scope selection**: Public read requests use structured `scope` JSON. Variants are `path`, `cwd_inferred`, `subtree`, `set`, and `all`. `cx_recall` accepts `path` and `cwd_inferred`; `cx_search` accepts the full vocabulary.

**Persisted scope path**: stored entries, exports, response payloads, and internal exact path models include a `scope_path` field that identifies where each row lives.

**Two-phase retrieval**: `cx_recall` and `cx_browse` return metadata + snippet. `cx_get` fetches full body content. This keeps initial responses compact.

**Soft-delete model**: Entries are never hard-deleted. `cx_forget` marks entries with a self-referential `superseded_by`. Replacement entries link to the entry they replace.

## Architecture

Five Rust crates:

| Crate | Role |
|-------|------|
| `cm-core` | Domain types, `ContextStore` trait, query logic. Zero I/O. |
| `cm-store` | SQLite adapter (sqlx). Schema, migrations, config, connection pooling. |
| `cm-capabilities` | Shared request/response types, validation, projections between adapters. |
| `cm-cli` | CLI binary (`cm`) and MCP server (JSON-RPC over stdio). |
| `cm-web` | Web monitoring dashboard (Axum + React/Vite). |

Tool documentation lives in `tools.toml`. `build.rs` generates MCP schema, CLI help text, and skill docs from that single source.

## MCP Tools

| Tool | Purpose |
|------|---------|
| `cx_recall` | Priority retrieval for one known scope via ancestor walk. |
| `cx_search` | FTS5 BM25 content search across wide or unknown scopes. |
| `cx_store` | Create an entry with auto-scope creation. |
| `cx_deposit` | Batch-store conversation exchanges. |
| `cx_browse` | Filtered inventory with cursor pagination. |
| `cx_get` | Full content by ID (phase 2 of two-phase retrieval). |
| `cx_update` | Partial update. Recomputes hash on body/kind change. |
| `cx_forget` | Soft-delete. |
| `cx_stats` | Aggregate statistics and scope tree. |
| `cx_export` | JSON export for backup/migration. |

## Entry Kinds

`feedback` (highest priority), `fact`, `decision`, `preference`, `lesson`, `reference`, `pattern`, `observation`.

## Quick Start

```sh
just build     # cargo build --workspace
just test      # cargo nextest run --workspace
just check     # fmt + clippy + web lint
just serve-dev # run MCP server locally
just web       # start web dashboard (backend + frontend)
```

Database: `~/.context-matters/cm.db`
Config: `~/.context-matters/.cm.config.toml`

## Distribution

npm package (`context-matters`) wraps the Rust binary. `npx -y context-matters serve` downloads the platform binary from GitHub Releases with SHA-256 verification.

Registered as `cm` server in `helioy-tools/.mcp.json`. Tools appear as `mcp__plugin_helioy-tools_cm__cx_*` in Claude Code.
