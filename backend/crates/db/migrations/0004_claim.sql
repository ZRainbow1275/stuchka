-- 0004_claim.sql — arbitration claim with state machine (data/01 §1.5.5 SQLite fallback).
-- UUID->TEXT; two ENUMs->TEXT+CHECK IN; amounts NUMERIC(14,2)->TEXT (fixed-point decimal string,
-- REAL forbidden for money — common hard constraint). Non-null amounts checked as legal non-negative
-- fixed-point. calculation_breakdown JSONB->TEXT; law_refs/fact_refs UUID[]->TEXT (JSON arrays);
-- inv10_withdraw_consistent CHECK kept verbatim. FK case_id ON DELETE RESTRICT (data/01 §1.8).
--
-- Amount shape CHECK (tightened, cross-crate-reconciliation D / task §2): a non-null amount must
--   (a) contain ONLY digits and the dot — NOT GLOB '*[^0-9.]*' rejects sign, spaces, letters,
--       scientific notation, currency symbols, etc.;
--   (b) start with a digit — GLOB '[0-9]*' rejects a leading dot / empty string; and
--   (c) hold AT MOST one decimal point — NOT GLOB '*.*.*' rejects '1.2.3'.
-- This is a coarse shape gate only; codec::text_to_decimal (rust_decimal) remains the authoritative
-- parser/validator. REAL is forbidden for money so amounts are always TEXT.
CREATE TABLE claim (
    id                          TEXT PRIMARY KEY,
    case_id                     TEXT NOT NULL REFERENCES "case"(id) ON DELETE RESTRICT,
    claim_type                  TEXT NOT NULL CHECK (claim_type IN (
                                  'economic_compensation','economic_damage','double_wage_no_contract',
                                  'overtime_pay','malicious_arrears_surcharge','work_injury_benefit','other')),
    amount_pre_tax              TEXT NULL CHECK (amount_pre_tax IS NULL OR (
                                  amount_pre_tax NOT GLOB '*[^0-9.]*'
                                  AND amount_pre_tax GLOB '[0-9]*'
                                  AND amount_pre_tax NOT GLOB '*.*.*')),
    amount_post_tax             TEXT NULL CHECK (amount_post_tax IS NULL OR (
                                  amount_post_tax NOT GLOB '*[^0-9.]*'
                                  AND amount_post_tax GLOB '[0-9]*'
                                  AND amount_post_tax NOT GLOB '*.*.*')),
    calculation_breakdown       TEXT NOT NULL,                  -- JSON
    status                      TEXT NOT NULL DEFAULT 'draft' CHECK (status IN (
                                  'draft','finalized','granted','denied','withdrawn')),
    law_refs                    TEXT NOT NULL DEFAULT '[]',     -- JSON array of UUID strings
    fact_refs                   TEXT NOT NULL DEFAULT '[]',     -- JSON array of UUID strings
    withdraw_inv10_confirmed    INTEGER NOT NULL DEFAULT 0 CHECK (withdraw_inv10_confirmed IN (0,1)),
    created_at                  TEXT NOT NULL,
    updated_at                  TEXT NOT NULL,
    CHECK (status <> 'withdrawn' OR withdraw_inv10_confirmed = 1)  -- inv10_withdraw_consistent (INV-10)
);

CREATE INDEX idx_claim_case ON claim(case_id, status);
