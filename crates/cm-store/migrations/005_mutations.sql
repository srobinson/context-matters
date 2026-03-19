-- Migration 005: Mutation history table
--
-- Records every entry mutation on the context store.
-- Written by cm-store in the same transaction as the mutation.
-- Snapshots are full JSON serializations of the entry at that point in time.
--
-- No FK on entry_id. Mutation records are self-contained (full snapshots)
-- and must survive even if an entry row is hard-deleted by maintenance,
-- import/reset, or future operations. An audit trail that disappears when
-- its subject is deleted defeats the purpose.

CREATE TABLE mutations (
    id               TEXT PRIMARY KEY,           -- UUID v7
    entry_id         TEXT NOT NULL,              -- Entry that was mutated (no FK, survives deletion)
    action           TEXT NOT NULL CHECK (action IN ('create', 'update', 'forget', 'supersede')),
    source           TEXT NOT NULL CHECK (source IN ('mcp', 'cli', 'web', 'helix')),
    timestamp        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    before_snapshot  TEXT,                       -- JSON: full entry state before (NULL for create)
    after_snapshot   TEXT                        -- JSON: full entry state after
);

-- Primary access pattern: "show me all mutations for this entry" (per-entry history)
CREATE INDEX idx_mutations_entry ON mutations(entry_id);

-- Secondary: "show me recent mutations across all entries" (activity feed, cm-web dashboard)
CREATE INDEX idx_mutations_timestamp ON mutations(timestamp);

-- Tertiary: "show me all mutations from a specific source" (filter by origin)
CREATE INDEX idx_mutations_source ON mutations(source);
