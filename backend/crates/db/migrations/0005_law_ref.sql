-- 0005_law_ref.sql — law reference snapshot + case association (data/01 §1.4.4/§1.4.5 SQLite fallback).
-- UUID->TEXT; stable_id LIKE 'law:%' (D8 URN) + content_hash length CHECK kept; used_in CHECK IN kept;
-- UNIQUE(stable_id,content_hash) kept; composite PK (case_id,law_ref_id,used_in) kept.
-- FK case_id ON DELETE RESTRICT, law_ref_id restrict-delete (data/01 §1.8).
CREATE TABLE law_ref (
    id                TEXT PRIMARY KEY,
    stable_id         TEXT NOT NULL CHECK (stable_id LIKE 'law:%'),
    content_hash      TEXT NOT NULL CHECK (length(content_hash) = 64),
    kb_version_label  TEXT NOT NULL,
    title             TEXT NOT NULL,
    version_date      TEXT NOT NULL,                            -- 'YYYY-MM-DD'
    article           TEXT NOT NULL,
    body_snapshot     TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    UNIQUE (stable_id, content_hash)
);

-- case <-> law_ref snapshot association (frozen at case freeze)
CREATE TABLE case_law_ref (
    case_id       TEXT NOT NULL REFERENCES "case"(id) ON DELETE RESTRICT,
    law_ref_id    TEXT NOT NULL REFERENCES law_ref(id) ON DELETE RESTRICT,
    used_in       TEXT NOT NULL CHECK (used_in IN ('diagnosis','document','calculation')),
    PRIMARY KEY (case_id, law_ref_id, used_in)
);

CREATE INDEX idx_law_ref_stable ON law_ref(stable_id);
CREATE INDEX idx_case_law_ref_case ON case_law_ref(case_id);
