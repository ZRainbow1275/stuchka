-- 0001_case.sql — case root aggregate (data/01 §1.1.5 SQLite fallback, backend/02 §2.2 + §2.11).
-- D2 SQLite fallback mapping: UUID->TEXT(36), ENUM->TEXT+CHECK IN, TIMESTAMPTZ->TEXT(RFC3339),
-- DATE->TEXT('YYYY-MM-DD'). Single-direction migration: no down.sql (backend/02 §2.13).
-- State-machine whitelist is enforced in the application layer (crates/db/src/repo/case.rs)
-- calling data-model::validate_case_transition; INV-04 kb_version_hash freeze likewise (§2.8).
CREATE TABLE "case" (
    id                   TEXT PRIMARY KEY,
    identity_type        TEXT NOT NULL CHECK (identity_type IN (
                           'standard_full_time','dispatch','part_time','new_employment',
                           'domestic_service','construction_labor','individual_employee',
                           'intern','retired_rehired','contractor','de_facto_no_contract')),
    dispute_subtype      TEXT NOT NULL CHECK (dispute_subtype IN (
                           'social_ins_waiver_invalid','social_ins_arrears','social_ins_underpaid_base',
                           'social_ins_intermittent','social_ins_proxy','social_ins_uninsured_injury',
                           'social_ins_cross_period','social_ins_base_dispute')),
    dispute_category     TEXT NOT NULL CHECK (length(dispute_category) = 8 AND dispute_category GLOB 'LD-[0-9][0-9]-[0-9][0-9]'),
    coverage_tier        TEXT NOT NULL DEFAULT 'make_usable' CHECK (coverage_tier IN ('make_deep','make_usable')),
    case_occurred_at     TEXT NOT NULL,                       -- 'YYYY-MM-DD'
    province             TEXT NOT NULL CHECK (province GLOB '[0-9][0-9]'),
    city                 TEXT NOT NULL CHECK (city GLOB '[0-9][0-9][0-9][0-9]'),
    region_code          TEXT NULL CHECK (region_code IS NULL OR region_code GLOB '[0-9][0-9][0-9][0-9][0-9][0-9]'),
    kb_version_hash      TEXT NOT NULL CHECK (length(kb_version_hash) = 64),
    kb_version_label     TEXT NOT NULL,
    status               TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','diagnosed','confirmed','frozen','disputed')),
    group_id             TEXT NULL,
    dialogue_template_id TEXT NULL,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL,
    frozen_at            TEXT NULL,
    CHECK ((status = 'frozen' AND frozen_at IS NOT NULL) OR (status <> 'frozen' AND frozen_at IS NULL)),
    CHECK (region_code IS NULL OR (substr(region_code,1,4) = city AND substr(region_code,1,2) = province))
);

CREATE INDEX idx_case_status ON "case"(status);
CREATE INDEX idx_case_group ON "case"(group_id) WHERE group_id IS NOT NULL;
CREATE INDEX idx_case_region ON "case"(province, city);
