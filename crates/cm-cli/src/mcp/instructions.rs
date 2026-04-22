//! Server instructions advertised through the MCP initialize response.

pub(super) const SERVER_INSTRUCTIONS: &str = "\
You have a structured context store for persistent project knowledge across sessions.

TASK WORKFLOW:
1. RECALL: After receiving a task, call cx_recall with a summary of what you are working on. \
   This returns relevant context entries (facts, decisions, preferences, lessons) from \
   the current scope and all ancestor scopes. Use returned context silently. \
   cx_recall is useful at any point during a session, not only after the initial task.
2. STORE: When you discover important facts, decisions, user preferences, lessons learned, \
   or recurring patterns, call cx_store to persist them. Classify entries by kind for \
   effective retrieval later.
3. FEEDBACK: When the user corrects you or clarifies a preference, store it as kind='feedback'. \
   Feedback entries receive highest recall priority.

TOOLS OVERVIEW:
- cx_recall: Search and retrieve context. Primary retrieval tool. Call after receiving a task.
- cx_store: Store a single context entry with structured metadata.
- cx_deposit: Batch-store conversation exchanges for future reference.
- cx_browse: List entries with filters and pagination. Defaults to inferred local scope.
- cx_get: Fetch full content for specific entry IDs (two-phase retrieval).
- cx_update: Partially update an existing entry.
- cx_forget: Soft-delete entries that are no longer relevant.
- cx_stats: View store statistics and scope breakdown.
- cx_export: Export entries as JSON for backup.

SCOPE MODEL:
Scopes form a hierarchy: global > project > repo > session. \
Context at broader scopes is visible at narrower scopes. \
When storing entries, use the narrowest appropriate scope. \
Global scope is for cross-project knowledge (user preferences, universal patterns). \
Project scope is for project-level decisions and conventions. \
Repo scope is for codebase-specific facts. \
Session scope is for ephemeral task context.

PRINCIPLES:
- Be selective. Store genuinely reusable knowledge, not routine observations.
- Classify accurately. The kind field drives recall priority and filtering.
- Use specific scope paths. Overly broad scoping pollutes recall for unrelated work.
- Do not mention the context system to the user unless asked.
- If cx_recall returns empty results, that is fine. The scope is new.";
