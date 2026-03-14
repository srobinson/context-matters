-- Migration 004: Add tags column to FTS5 index
--
-- Rebuilds entries_fts as a contentless FTS5 table with a tags column.
-- Contentless mode (content='') is required because tags are extracted from
-- the JSONB meta column, not a real column on the entries table. Content-sync
-- mode (content='entries') would fail to read a non-existent tags column.
--
-- Triggers handle all sync. The entries_updated_at trigger from migration 003
-- is left untouched.

-- Drop existing FTS triggers
DROP TRIGGER IF EXISTS entries_fts_insert;
DROP TRIGGER IF EXISTS entries_fts_delete;
DROP TRIGGER IF EXISTS entries_fts_update;

-- Drop old FTS table
DROP TABLE IF EXISTS entries_fts;

-- Recreate as contentless with tags column
CREATE VIRTUAL TABLE entries_fts USING fts5(
    title, body, tags,
    content='',
    tokenize='porter unicode61'
);

-- INSERT trigger: extract tags from meta JSONB
CREATE TRIGGER entries_fts_insert AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, title, body, tags)
    VALUES (
        new.rowid,
        new.title,
        new.body,
        COALESCE((SELECT group_concat(j.value, ' ') FROM json_each(json_extract(new.meta, '$.tags')) j), '')
    );
END;

-- DELETE trigger: remove old values from contentless index
CREATE TRIGGER entries_fts_delete AFTER DELETE ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, title, body, tags)
    VALUES (
        'delete',
        old.rowid,
        old.title,
        old.body,
        COALESCE((SELECT group_concat(j.value, ' ') FROM json_each(json_extract(old.meta, '$.tags')) j), '')
    );
END;

-- UPDATE trigger: scoped to title, body, meta to avoid recursion with
-- the entries_updated_at trigger (which only touches updated_at).
-- Adding meta to the OF clause ensures tag changes are reflected in the index.
CREATE TRIGGER entries_fts_update AFTER UPDATE OF title, body, meta ON entries BEGIN
    INSERT INTO entries_fts(entries_fts, rowid, title, body, tags)
    VALUES (
        'delete',
        old.rowid,
        old.title,
        old.body,
        COALESCE((SELECT group_concat(j.value, ' ') FROM json_each(json_extract(old.meta, '$.tags')) j), '')
    );
    INSERT INTO entries_fts(rowid, title, body, tags)
    VALUES (
        new.rowid,
        new.title,
        new.body,
        COALESCE((SELECT group_concat(j.value, ' ') FROM json_each(json_extract(new.meta, '$.tags')) j), '')
    );
END;

-- Repopulate index from existing data
INSERT INTO entries_fts(rowid, title, body, tags)
SELECT
    e.rowid,
    e.title,
    e.body,
    COALESCE((SELECT group_concat(j.value, ' ') FROM json_each(json_extract(e.meta, '$.tags')) j), '')
FROM entries e;
