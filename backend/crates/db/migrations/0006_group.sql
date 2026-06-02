-- 0006_group.sql — R1 async group case (data/01 §1.6.2/§1.6.3 SQLite fallback).
-- "group" is a reserved word -> double-quoted. UUID->TEXT; authorization_chain JSONB->TEXT;
-- composite PK (group_id,contributor_id) + FK ON DELETE CASCADE kept (needs PRAGMA foreign_keys=ON).
-- "group".case_id FK ON DELETE RESTRICT (data/01 §1.8).
CREATE TABLE "group" (
    id                  TEXT PRIMARY KEY,
    case_id             TEXT NOT NULL REFERENCES "case"(id) ON DELETE RESTRICT,
    initiator_id        TEXT NOT NULL,
    inv10_confirmed_at  TEXT NOT NULL,
    created_at          TEXT NOT NULL
);

CREATE TABLE group_contributor (
    group_id            TEXT NOT NULL REFERENCES "group"(id) ON DELETE CASCADE,
    contributor_id      TEXT NOT NULL,
    authorization_chain TEXT NOT NULL,                          -- JSON
    joined_at           TEXT NOT NULL,
    PRIMARY KEY (group_id, contributor_id)
);

CREATE INDEX idx_group_case ON "group"(case_id);
