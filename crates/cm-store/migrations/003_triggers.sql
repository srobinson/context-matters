-- Migration 003: FTS5 content-sync triggers and updated_at trigger
--
-- FTS5 content-sync triggers keep entries_fts in lockstep with entries.
-- The UPDATE trigger must delete using OLD values before inserting NEW values.
-- Getting this order wrong silently corrupts the FTS index.
--
-- Critical interaction: the entries_updated_at trigger does an UPDATE on
-- entries to set updated_at. Without guards, this re-fires entries_fts_update,
-- causing FTS5 CORRUPT_VTAB (error 267). Two mitigations:
--   1. FTS update trigger scoped to AFTER UPDATE OF title, body
--   2. updated_at trigger guarded with WHEN old.updated_at = new.updated_at

-- Sync FTS on INSERT
CREATE TRIGGER entries_fts_insert AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, title, body)
    VALUES (new.rowid, new.title, new.body);
END;

-- Sync FTS on DELETE
CREATE TRIGGER entries_fts_delete AFTER DELETE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, title, body)
    VALUES ('delete', old.rowid, old.title, old.body);
END;

-- Sync FTS on UPDATE of indexed columns only.
-- Scoped to title/body to avoid firing when entries_updated_at modifies
-- the updated_at column, which would double-fire and corrupt the FTS index.
CREATE TRIGGER entries_fts_update AFTER UPDATE OF title, body ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, title, body)
    VALUES ('delete', old.rowid, old.title, old.body);
    INSERT INTO entries_fts(rowid, title, body)
    VALUES (new.rowid, new.title, new.body);
END;

-- Auto-maintain updated_at on any entry update.
-- WHEN guard prevents infinite recursion: the trigger's own UPDATE changes
-- updated_at, so re-checking old.updated_at = new.updated_at will be false
-- on the recursive invocation, stopping the chain.
CREATE TRIGGER entries_updated_at AFTER UPDATE ON entries
WHEN old.updated_at = new.updated_at BEGIN
    UPDATE entries SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
    WHERE id = new.id;
END;
