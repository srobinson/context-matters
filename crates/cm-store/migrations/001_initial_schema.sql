-- Migration 001: Core tables (scopes, entries, entry_relations)
--
-- Creates the three primary tables and all indexes.
-- FK constraints enforce referential integrity:
--   - scopes.parent_path: RESTRICT (cannot delete a scope with children)
--   - entries.scope_path: RESTRICT (cannot delete a scope with entries)
--   - entries.superseded_by: SET NULL (removing replacement reactivates original)
--   - entry_relations source/target: CASCADE (removing entry cleans up relations)

-- Scope hierarchy
CREATE TABLE scopes (
    path        TEXT PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN ('global', 'project', 'repo', 'session')),
    label       TEXT NOT NULL,
    parent_path TEXT,
    meta        TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (parent_path) REFERENCES scopes(path) ON DELETE RESTRICT ON UPDATE RESTRICT
);

-- Primary context store
CREATE TABLE entries (
    id              TEXT PRIMARY KEY,
    scope_path      TEXT NOT NULL,
    kind            TEXT NOT NULL,
    title           TEXT NOT NULL,
    body            TEXT NOT NULL,
    content_hash    TEXT NOT NULL,
    meta            TEXT,
    created_by      TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    superseded_by   TEXT,
    FOREIGN KEY (scope_path) REFERENCES scopes(path) ON DELETE RESTRICT ON UPDATE RESTRICT,
    FOREIGN KEY (superseded_by) REFERENCES entries(id) ON DELETE SET NULL ON UPDATE CASCADE
);

-- Entry indexes
CREATE INDEX idx_entries_scope ON entries(scope_path);
CREATE INDEX idx_entries_kind ON entries(kind);
CREATE INDEX idx_entries_scope_kind ON entries(scope_path, kind);
CREATE INDEX idx_entries_updated ON entries(updated_at);
CREATE INDEX idx_entries_content_hash ON entries(content_hash);
CREATE INDEX idx_entries_superseded ON entries(superseded_by);

-- Cross-references between entries
CREATE TABLE entry_relations (
    source_id   TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    relation    TEXT NOT NULL CHECK (relation IN (
        'supersedes', 'relates_to', 'contradicts', 'elaborates', 'depends_on'
    )),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (source_id, target_id, relation),
    FOREIGN KEY (source_id) REFERENCES entries(id) ON DELETE CASCADE ON UPDATE CASCADE,
    FOREIGN KEY (target_id) REFERENCES entries(id) ON DELETE CASCADE ON UPDATE CASCADE
);

CREATE INDEX idx_relations_target ON entry_relations(target_id);
