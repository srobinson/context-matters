-- Migration 002: FTS5 virtual table for full-text search
--
-- Content-sync mode: the FTS index mirrors the entries table via triggers
-- (created in migration 003). The content= and content_rowid= options tell
-- FTS5 to read from the entries table using its implicit integer rowid.
--
-- Tokenizer: porter stemmer + unicode61 for case/accent folding.

CREATE VIRTUAL TABLE entries_fts USING fts5(
    title, body,
    content='entries',
    content_rowid='rowid',
    tokenize='porter unicode61'
);
