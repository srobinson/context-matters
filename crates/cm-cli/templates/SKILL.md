---
name: cm
description: >
  Structured context store for AI agents. Use before any session to recall
  relevant knowledge, and during work to persist facts, decisions, preferences,
  and lessons. All tools are prefixed `cx_*`. Trigger when: starting a session,
  discovering reusable knowledge, receiving user corrections, or ending a session
  with conversation deposits.
---

# Context Matters: Structured Context Store

This project has a structured context store available via the **`cm` MCP server**. All tools are prefixed `cx_*`. Use them to persist and retrieve project knowledge across sessions.

## MCP Tools

| Tool | Purpose | Example |
|------|---------|---------|
| `cx_recall` | Priority context for one known scope | `cx_recall(query: "auth decisions", scope: {"kind":"path","path":"global/project:helioy"})` |
| `cx_search` | Content search across wide or unknown scopes | `cx_search(query: "auth decisions", scope: {"kind":"all"})` |
| `cx_store` | Persist a fact, decision, preference, or lesson | `cx_store(title: "Use UUIDv7", body: "...", kind: "decision")` |
| `cx_deposit` | Batch-store conversation exchanges | `cx_deposit(exchanges: [{user: "...", assistant: "..."}])` |
| `cx_browse` | List entries with filters and pagination | `cx_browse(kind: "decision", scope: {"kind":"path","path":"global/project:helioy"})` |
| `cx_get` | Fetch full content for specific entry IDs | `cx_get(ids: ["uuid1", "uuid2"])` |
| `cx_update` | Partially update an existing entry | `cx_update(id: "uuid", title: "Updated title")` |
| `cx_forget` | Mark entries forgotten so active reads skip them | `cx_forget(ids: ["uuid"])` |
| `cx_stats` | View store statistics and scope breakdown | `cx_stats()` |
| `cx_export` | Export entries as JSON for backup | `cx_export(scope: "global/project:helioy")` |

## Context Management Workflow

1. **Recall**: `cx_recall(query: "topic")` retrieves priority context for one known scope by walking ancestors.
2. **Search**: `cx_search(query: "topic", scope: {"kind":"all"})` searches across wide or unknown scopes.
3. **Store**: `cx_store(title, body, kind)` persists facts, decisions, preferences, lessons.
4. **Deposit**: `cx_deposit(exchanges)` batch stores conversation exchanges for future reference.
5. **Browse**: `cx_browse(kind: "decision")` lists entries with filters and pagination.
6. **Get**: `cx_get(ids)` fetches full content for specific entries.
7. **Update**: `cx_update(id, title: "new")` partially updates an existing entry.
8. **Forget**: `cx_forget(ids)` marks entries forgotten so active reads skip them.

### Task Workflow

```
1. Receive task from user or orchestrator
2. cx_recall(query: "summary of task", scope: {"kind":"path","path":"global/project:helioy/repo:nancyr"})
   retrieve priority context for this known scope and its ancestors
3. Work on the task
4. cx_store(title: "...", body: "...", kind: "decision", scope: "global/project:helioy/repo:nancyr")
   persist reusable knowledge when discovered
5. cx_deposit(exchanges: [...], summary: "...")
   preserve conversation at session end for continuity
```

Key: use cx_recall for a single known scope. Use cx_search when the right scope is unknown, broad, or cross-repo.
When the user corrects you, store it immediately as kind: "feedback" (highest recall priority).

### Scope Model

Scopes form a hierarchy. Context at broader scopes is visible at narrower scopes.

```text
global                                           cross-project knowledge
global/project:helioy                           project-level decisions
global/project:helioy/repo:context-matters      codebase-specific facts
global/project:helioy/repo:context-matters/session:abc   task context
```

Public requests select scope through the `scope` field. The canonical scope path string returned by read tools can be passed directly to write tools.

### Singular Selectors

| Selector | Request example |
|----------|-----------------|
| canonical path string | `{ "scope": "global/project:helioy/repo:context-matters" }` |
| path | `{ "scope": { "kind": "path", "path": "global/project:helioy/repo:context-matters" } }` |
| cwd_inferred | `{ "scope": { "kind": "cwd_inferred", "cwd": "/path/to/repo" } }` |
| project | `{ "scope": { "kind": "project", "project": "helioy" } }` |
| repo | `{ "scope": { "kind": "repo", "project": "helioy", "repo": "context-matters" } }` |
| session | `{ "scope": { "kind": "session", "project": "helioy", "repo": "context-matters", "session": "abc" } }` |

### Broad Selectors

| Selector | Request example |
|----------|-----------------|
| descendants, subtree alias accepted | `{ "scope": { "kind": "descendants", "path": "global/project:helioy" } }` |
| set | `{ "scope": { "kind": "set", "paths": ["global", "global/project:helioy"] } }` |
| all | `{ "scope": { "kind": "all" } }` |

### Tool Scope Boundaries

| Tool | Scope request |
|------|---------------|
| `cx_recall` | Optional singular scope selector |
| `cx_search` | Required singular or broad scope selector |
| `cx_store` | Optional singular scope selector |
| `cx_deposit` | Optional singular scope selector |
| `cx_browse` | Optional singular or broad scope selector |
| `cx_export` | Optional singular or broad scope selector |

For cwd based browse resolution, call `cx_browse(scope: {"kind":"cwd_inferred","cwd":"/path/to/repo"})`.
`cwd_inferred` resolves linked git worktrees to the source repository identity. Persisted entries and export rows include `scope_path`. Read projections use `scope` for compact row output.

### Two-Phase Retrieval

`cx_recall`, `cx_search`, `cx_browse` return metadata plus snippets. Use `cx_get` with returned IDs to fetch full body content.
This keeps initial responses compact while allowing selective deep reads.


## Parameter Reference

> Auto-generated from tools.toml.

### `cx_recall`

Recall priority context for a single known scope by walking that scope and its ancestors. Call after receiving a task with a summary of what you are working on. With a query, uses FTS5 inside the ancestor walk. Without a query, returns all entries visible at the target scope. Use cx_search when you need content search across descendants, set, or all scopes. Returns metadata + snippet for two-phase retrieval; use cx_get for full body. IMPORTANT: The query uses FTS5 with implicit AND between words. Use 1-3 keywords, not full sentences. More words = fewer results. Examples: 'auth migration' (good), 'how does the authentication migration work' (too many words, likely 0 results). Use OR for alternatives: 'auth OR authentication'. Use prefix matching: 'migrat*'.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | no | FTS5 search query. Use 1-3 keywords (implicit AND); keywords match, full sentences match nothing. Supports prefix que... |
| `scope` | string | object | no | Singular scope input for recall. Pass a canonical scope path string returned by read tools, or a structured singular ... |
| `kinds` | array<string> | no | Filter to specific entry kinds (OR logic). Valid values: fact, decision, preference, lesson, reference, feedback, pat... |
| `tags` | array<string> | no | Filter to entries with any of these tags (OR logic). Pass a JSON array: ["tag1", "tag2"]. |
| `limit` | integer | no | Maximum number of entries to return. Default: 20, max: 200. |
| `max_tokens` | integer | no | Maximum token budget for the response. Results are trimmed to fit within this budget, prioritizing higher-relevance e... |

### `cx_search`

Search cm entries by content across scopes. Returns FTS5 BM25-ranked hits. Use cx_search when you have a query and want results from multiple scopes, an unknown scope, or all scopes. Use cx_recall when you want priority-ordered context for a single known scope, walking ancestors. Recall is sharper but narrower.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | yes | Required FTS5 search query. Use 1-3 keywords. Supports prefix queries (rust*), phrase queries ("scope path"), and boo... |
| `scope` | string | object | yes | Scope input for content search. Pass a canonical scope path string, a singular selector such as cwd_inferred, project... |
| `kinds` | array<string> | no | Filter to specific entry kinds (OR logic). Valid values: fact, decision, preference, lesson, reference, feedback, pat... |
| `tags` | array<string> | no | Filter to entries with any of these tags (OR logic). Pass a JSON array: ["tag1", "tag2"]. |
| `limit` | integer | no | Maximum number of entries to return. Default: 20, max: 200. |
| `cursor` | string | no | Opaque pagination cursor from a previous cx_search response. Pass next_cursor to fetch the next page. |

### `cx_store`

Store a single context entry with structured metadata. Scopes are auto-created if they do not exist. Use 'supersedes' to replace an existing entry by marking the old one inactive. Returns the new entry ID and content hash.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `title` | string | yes | Short summary of the entry. Displayed in search results and browse listings. |
| `body` | string | yes | Full content body in markdown. |
| `kind` | enum: fact \| decision \| preference \| lesson \| reference \| feedback \| pattern \| observation | yes | Entry classification. Determines recall priority. fact: verified information. decision: architectural choice with rat... |
| `scope` | string | object | no | Singular write scope. Pass a canonical scope path string returned by read tools, or a structured singular selector: p... |
| `created_by` | string | no | Attribution string. Format: 'source_type:identifier'. Examples: 'human:stuart', 'agent:claude-code', 'system:consolid... |
| `tags` | array<string> | no | Freeform tags for categorization and filtering. Pass a JSON array: ["tag1", "tag2"]. |
| `confidence` | enum: high \| medium \| low | no | Confidence level. Affects recall priority ordering: high entries surface before low entries at the same scope level. |
| `source` | string | no | Source URL or file path for reference entries. |
| `expires_at` | string | no | ISO 8601 expiry timestamp. After this time the entry is considered stale. Stored but not enforced by the storage layer. |
| `priority` | integer | no | Numeric priority for manual ordering. Higher values surface first in recall results. |
| `supersedes` | string | no | ID of an existing entry that this new entry replaces. The old entry is marked inactive and linked via a 'supersedes' ... |

### `cx_deposit`

Batch-store conversation exchanges for future context. Each exchange (user/assistant pair) becomes an observation entry. Optional summary creates a linked observation with 'elaborates' relations to each exchange. All entries created in a single transaction. Maximum 50 exchanges per deposit.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `exchanges` | array<object> | yes | Conversation exchanges to store. Each exchange has 'user' (user message), 'assistant' (assistant response), and optio... |
| `summary` | string | no | Optional summary of the conversation. Stored as a separate observation entry linked to each exchange via 'elaborates'... |
| `scope` | string | object | no | Singular write scope. Pass a canonical scope path string returned by read tools, or a structured singular selector: p... |
| `created_by` | string | no | Attribution string. Default: 'agent:claude-code'. |

### `cx_browse`

List entries with filtering and cursor-based pagination. For inventory and exploration, not semantic search. Defaults to cwd_inferred when scope is omitted. Returns metadata + snippet (two-phase retrieval). Filters combine with AND semantics. Results ordered by updated_at DESC.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `scope` | string | object | no | Scope input for browse. Pass a canonical scope path string, a singular selector, or a broad selector: descendants, su... |
| `include_resolution` | boolean | no | Include scope resolution metadata in the response. Defaults to true when scope="cwd_inferred". |
| `kind` | enum: fact \| decision \| preference \| lesson \| reference \| feedback \| pattern \| observation | no | Filter by entry kind. |
| `tag` | string | no | Filter by tag. Entries must have at least one matching tag. |
| `created_by` | string | no | Filter by creator attribution string. |
| `include_superseded` | boolean | no | Include superseded (inactive) entries in results. Default: false. |
| `limit` | integer | no | Maximum entries per page. Default: 20, max: 200. |
| `cursor` | string | no | Opaque pagination cursor from a previous cx_browse response. Pass next_cursor to fetch the next page. |

### `cx_get`

Fetch full content for specific entry IDs. Phase 2 of two-phase retrieval. Use after cx_recall or cx_browse to load full body content. Accepts full hyphenated UUIDv7 strings only. IDs that do not exist are silently omitted. Maximum 100 IDs per request.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `ids` | array<string> | yes | Entry IDs: full hyphenated UUIDv7 strings. Max 100 per request. |

### `cx_update`

Partially update an existing entry. Only provided fields are modified. Changing body or kind recomputes content_hash and checks for duplicates. Scope migration is excluded; use cx_store with supersedes to move entries across scopes. At least one field must be provided.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `id` | string | yes | ID of the entry to update. |
| `title` | string | no | New title. Omit to keep existing. |
| `body` | string | no | New body content in markdown. Changing body recomputes content_hash and checks for duplicates. Omit to keep existing. |
| `kind` | enum: fact \| decision \| preference \| lesson \| reference \| feedback \| pattern \| observation | no | New kind classification. Changing kind recomputes content_hash. Omit to keep existing. |
| `meta` | object | no | Replace metadata entirely. Provide the complete desired meta object with fields: tags (array), confidence (high/mediu... |

### `cx_forget`

Mark entries as forgotten. Sets superseded_by to the entry's own ID, distinguishing forgotten entries from entries superseded by a replacement. Already-inactive entries are silently skipped. Maximum 100 IDs per request. Partial success is reported.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `ids` | array<string> | yes | Entry IDs to forget. Maximum 100 per request. Each ID is processed independently. |

### `cx_stats`

View aggregate statistics about the context store. Returns active/superseded entry counts, scope count, relation count, breakdown by kind, by scope, and by tag, database file size, and scope tree. Diagnostic tool for understanding what context exists.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `tag_sort` | enum: name \| count | no | Sort order for tag breakdown. 'name': alphabetical ascending (default). 'count': most used tags first. |

### `cx_export`

Export entries and scopes as JSON for backup or migration. Returns all active entries (superseded excluded) and matching scopes. Relations are excluded in v1. Optionally filter with a scope selector, including descendants for subtree backup.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `scope` | string | object | no | Filter export to a scope input. Pass a canonical scope path string for exact export, cwd_inferred for current scope, ... |
| `format` | enum: json | no | Export format. Currently only 'json' is supported. Default: 'json'. |

## Rules

1. **Call `cx_recall` after receiving a task** with a summary of what you are working on
2. **Store selectively**. Persist genuinely reusable knowledge, not routine observations
3. **Classify accurately**. The `kind` field drives recall priority and filtering
4. **Use specific `scope` selectors**. Broad selectors are reserved for `cx_search`, `cx_browse`, `cx_export`
5. **Two-phase retrieval**. Use `cx_get` when a snippet is not enough
6. **Store feedback immediately**. When the user corrects you, `kind: "feedback"` gets highest recall priority
7. **Do not mention the context system** to the user unless asked

## CLI Fallback

If MCP tools are unavailable, use the CLI directly:

```bash
cm stats     # Show store statistics
cm serve     # Start MCP server on stdio
```
