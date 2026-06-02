-- 0002_fact.sql — fact first-class object (data/01 §1.2.5 SQLite fallback).
-- UUID->TEXT; four ENUMs->TEXT+CHECK IN; confidence REAL (0-1 CHECK kept);
-- evidence_refs UUID[]->TEXT (JSON array); authorization_chain JSONB->TEXT;
-- ai_must_have_confidence CHECK kept verbatim.
-- FK case_id ON DELETE RESTRICT (data/01 §1.8). PRAGMA foreign_keys=ON set per connection.
CREATE TABLE fact (
    id                   TEXT PRIMARY KEY,
    case_id              TEXT NOT NULL REFERENCES "case"(id) ON DELETE RESTRICT,
    content              TEXT NOT NULL,
    category             TEXT NOT NULL CHECK (category IN (
                           'relation_qualification','wage','time_period','termination_reason',
                           'work_injury','discrimination','other')),
    status               TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','confirmed','disputed','deprecated')),
    source               TEXT NOT NULL CHECK (source IN ('user_input','ai_inferred','rule_engine')),
    confidence           REAL NULL CHECK (confidence IS NULL OR (confidence >= 0 AND confidence <= 1)),
    coverage_tag         TEXT NOT NULL CHECK (coverage_tag IN ('exact','approximate','boundary','unknown')),
    evidence_refs        TEXT NOT NULL DEFAULT '[]',          -- JSON array of UUID strings
    group_id             TEXT NULL,
    contributor_id       TEXT NULL,
    authorization_chain  TEXT NULL,                            -- JSON or NULL
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL,
    CHECK (source <> 'ai_inferred' OR confidence IS NOT NULL)  -- ai_must_have_confidence
);

CREATE INDEX idx_fact_case ON fact(case_id, status);
CREATE INDEX idx_fact_group ON fact(group_id) WHERE group_id IS NOT NULL;
CREATE INDEX idx_fact_contributor ON fact(contributor_id) WHERE contributor_id IS NOT NULL;
