-- Migration 006: Recall ranking shadow canary
--
-- Observe-only diff rows for recall ranking experiments. The table is
-- self-contained and deliberately stores IDs plus query hashes only.
-- Never store raw query, title, body, or snippets here.

CREATE TABLE recall_shadow (
    id                       TEXT PRIMARY KEY, -- UUID v7
    ts                       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    scope_path               TEXT,
    query_hash               TEXT,
    query_len                INTEGER,
    routing                  TEXT NOT NULL,
    tier                     TEXT,
    k                        INTEGER NOT NULL,
    candidate_count          INTEGER NOT NULL,
    top1_changed             INTEGER NOT NULL,
    topk_overlap             REAL NOT NULL,
    footrule                 REAL NOT NULL,
    mean_abs_position_delta  REAL NOT NULL,
    position_deltas          TEXT NOT NULL,
    old_ids                  TEXT NOT NULL,
    new_ids                  TEXT NOT NULL,
    window_truncated         INTEGER NOT NULL,
    ranking_version          TEXT NOT NULL,
    duration_ms              INTEGER NOT NULL
);

CREATE INDEX idx_recall_shadow_ts ON recall_shadow(ts);
CREATE INDEX idx_recall_shadow_top1_changed ON recall_shadow(top1_changed);
CREATE INDEX idx_recall_shadow_routing ON recall_shadow(routing);
CREATE INDEX idx_recall_shadow_scope_path ON recall_shadow(scope_path);
