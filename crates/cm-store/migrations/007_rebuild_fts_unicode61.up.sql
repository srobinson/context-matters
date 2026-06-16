-- Migration 007: rebuild FTS5 with exact unicode61 tokenization
--
-- Porter stemming collapses short acronyms such as vps -> vp and ios -> io.
-- Rebuild the contentless index with unicode61 only while preserving the
-- post-004 columns, rowid mapping, tag extraction, triggers, and backfill.

DROP TRIGGER IF EXISTS entries_fts_insert;
DROP TRIGGER IF EXISTS entries_fts_delete;
DROP TRIGGER IF EXISTS entries_fts_update;

DROP TABLE IF EXISTS entries_fts;

CREATE VIRTUAL TABLE entries_fts USING fts5(
    title, body, tags,
    content='',
    tokenize='unicode61'
);

CREATE TRIGGER entries_fts_insert AFTER INSERT ON entries BEGIN
    INSERT INTO entries_fts(rowid, title, body, tags)
    VALUES (
        new.rowid,
        new.title,
        new.body,
        COALESCE((SELECT group_concat(j.value, ' ') FROM json_each(json_extract(new.meta, '$.tags')) j), '')
    );
END;

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

INSERT INTO entries_fts(rowid, title, body, tags)
SELECT
    e.rowid,
    e.title,
    e.body,
    COALESCE((SELECT group_concat(j.value, ' ') FROM json_each(json_extract(e.meta, '$.tags')) j), '')
FROM entries e;
