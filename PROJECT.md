# context-matters

Structured context store for AI agents. Part of the helioy ecosystem.

## What It Does

Provides persistent, hierarchical, scoped context storage so AI agents can recall facts, decisions, preferences, and lessons across sessions. SQLite + FTS5 backend served as an MCP server over stdio.

## Architecture

Three-crate Rust workspace following the attention-matters pattern:

```
context-matters/
├── crates/
│   ├── cm-core/        # Domain types, ContextStore trait, query logic. Zero I/O.
│   ├── cm-store/       # SQLite adapter via sqlx. Schema, migrations, config.
│   └── cm-cli/         # CLI binary (`cm`) and MCP server.
├── npm/
│   └── context-matters/  # npm wrapper for npx distribution
├── tools.toml          # Single source of truth for tool + parameter docs
└── justfile            # Build/test/lint recipes
```

### Crate Responsibilities

**cm-core**: Pure domain logic. Defines `ContextStore` trait (sync signatures), all domain types (`Entry`, `Scope`, `EntryKind`, `ScopePath`, etc.), and query construction. Testable without a database.

**cm-store**: `SqliteContextStore` implements the `ContextStore` trait. Owns schema (3 SQL migrations), config resolution (TOML + env vars), connection pooling (1 write / 4 read), and all sqlx interaction. Async public API wraps sync trait methods.

**cm-cli**: Binary crate producing `cm`. Contains `McpServer` (manual JSON-RPC over stdio, same pattern as fmm) and CLI subcommands. `build.rs` reads `tools.toml` and generates MCP schema + CLI help constants.

## Key Design Decisions

- **Manual JSON-RPC over stdio** instead of rmcp. Keeps tool documentation in `tools.toml` (single source of truth for MCP, CLI, and skill docs). Proven pattern from fmm.
- **sqlx over rusqlite**. Compile-time query checking, built-in migration system, native async + connection pooling. The `ContextStore` trait in cm-core stays sync; async wrapping is cm-store's concern.
- **UUID v7** for entry IDs. Time-sortable, 1.9+ for monotonicity within the same millisecond.
- **BLAKE3 content hashing** for deduplication. Hash input: `scope_path + \0 + kind + \0 + body`. Title excluded.
- **Scope hierarchy**: `global > project > repo > session`. Ancestor walk provides context inheritance. Scopes auto-created by MCP tools.
- **Soft-delete via `superseded_by`**. Forgotten entries set `superseded_by = own ID`. Superseded entries point to replacement.
- **FTS5** with porter + unicode61 tokenizer. Content-sync triggers keep index in lockstep with entries table.

## MCP Tools (9 total, `cx_` prefix)

| Tool | Purpose |
|------|---------|
| `cx_recall` | Search + scope resolution. Primary retrieval. Two-phase (metadata + snippet). |
| `cx_store` | Create entry with auto-scope creation. Supports superseding. |
| `cx_deposit` | Batch-store conversation exchanges. |
| `cx_browse` | Filtered inventory with cursor pagination. |
| `cx_get` | Full content retrieval by ID (phase 2). |
| `cx_update` | Partial update. Recomputes hash on body/kind change. |
| `cx_forget` | Soft-delete (self-referential superseded_by). |
| `cx_stats` | Aggregate statistics and scope tree. |
| `cx_export` | JSON export for backup/migration. |

## Entry Kinds

| Kind | Recall Priority | Purpose |
|------|----------------|---------|
| `feedback` | Highest | User corrections and clarifications |
| `fact` | Normal | Verified information |
| `decision` | Normal | Architectural choices with rationale |
| `preference` | Normal | User/project preferences |
| `lesson` | Normal | Learned from experience |
| `reference` | Normal | Pointers to external material |
| `pattern` | Normal | Recurring code/process patterns |
| `observation` | Normal | General-purpose notes |

## Scope Model

```
global                                          — cross-project knowledge
global/project:helioy                           — project-level decisions
global/project:helioy/repo:nancyr               — codebase-specific facts
global/project:helioy/repo:nancyr/session:abc   — ephemeral task context
```

Scopes form a strict hierarchy. Context at broader scopes is visible at narrower scopes via ancestor walk. Intermediate levels can be omitted.

## Database

- Location: `~/.context-matters/cm.db`
- WAL mode with aggressive pragmas (64MB cache, 256MB mmap, 5s busy timeout)
- Dual pool: 1 write connection, 4 read connections
- Tables: `scopes`, `entries`, `entry_relations`, `entries_fts` (virtual)
- Migrations embedded at compile time via `sqlx::migrate!()`

## Distribution

npm package (`context-matters`) wraps the Rust binary. `npx -y context-matters serve` downloads the platform binary from GitHub Releases on postinstall, with SHA-256 verification.

## Plugin Integration

Registered as `cm` server in `helioy-tools/.mcp.json`. Tools appear as `mcp__plugin_helioy-tools_cm__cx_*` in Claude Code.

## Spec Documents

Foundational specifications live in `~/.mdx/projects/`:

- `context-matters-spec-core-types-and-trait.md` — All Rust types and ContextStore trait
- `context-matters-spec-schema-and-storage.md` — DDL, scope paths, hashing, concurrency
- `context-matters-spec-scaffold-and-integration.md` — Crate layout, dependencies, plugin registration
- `context-matters-spec-mcp-server-and-tools.md` — MCP server struct and all 9 cx_* tools
- `context-matters-tools.toml` — Tool documentation source of truth (also at repo root)
