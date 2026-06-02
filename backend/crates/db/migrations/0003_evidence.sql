-- 0003_evidence.sql — seven-class evidence (data/01 §1.3.5 SQLite fallback).
-- UUID->TEXT; two ENUMs->TEXT+CHECK IN; score_breakdown JSONB->TEXT (five-key check at app layer);
-- gps_coords JSONB->TEXT; chain_membership TEXT[]->TEXT (JSON array); UNIQUE(case_id,file_hash) kept.
-- GIN index dropped (no SQLite GIN); chain filtering at app layer (data/01 §1.3.6).
-- FK case_id ON DELETE RESTRICT (data/01 §1.8). INV-09 quarantine export-block at app layer.
CREATE TABLE evidence (
    id                TEXT PRIMARY KEY,
    case_id           TEXT NOT NULL REFERENCES "case"(id) ON DELETE RESTRICT,
    evidence_type     TEXT NOT NULL CHECK (evidence_type IN (
                        'documentary_contract','audio_video','digital_communication',
                        'witness_statement','scene_photo_video','third_party_data','appraisal')),
    file_path         TEXT NOT NULL,
    file_hash         TEXT NOT NULL CHECK (length(file_hash) = 64),
    mime_type         TEXT NOT NULL,
    byte_size         INTEGER NOT NULL CHECK (byte_size > 0 AND byte_size <= 104857600),  -- 100MB
    effective_score   REAL NOT NULL CHECK (effective_score BETWEEN 0 AND 1),
    score_breakdown   TEXT NOT NULL,                           -- JSON {source,timing,completeness,relevance,authenticity}
    status            TEXT NOT NULL DEFAULT 'uploaded' CHECK (status IN (
                        'uploaded','parsed','scored','verified','disputed','quarantined')),
    high_sensitivity  INTEGER NOT NULL DEFAULT 0 CHECK (high_sensitivity IN (0,1)),
    collected_at      TEXT NULL,
    device_id         TEXT NULL,
    gps_coords        TEXT NULL,                               -- JSON {lat,lon} or NULL
    chain_membership  TEXT NOT NULL DEFAULT '[]',              -- JSON array of strings
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    UNIQUE (case_id, file_hash)                                -- same-case dedup
);

CREATE INDEX idx_evidence_case_type ON evidence(case_id, evidence_type);
CREATE INDEX idx_evidence_score ON evidence(case_id, effective_score DESC);
CREATE INDEX idx_evidence_sha ON evidence(file_hash);
CREATE INDEX idx_evidence_quarantine ON evidence(case_id) WHERE status = 'quarantined';
