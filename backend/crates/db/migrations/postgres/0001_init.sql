-- Postgres DRAFT DDL — preserved for the R-phase reassessment (D2 §2.11 / backend/05 §5.6).
-- NOT executed by the R1 runner: R1 ships SQLite only (L0-03 spike conclusion, see research report).
-- dialect.rs intentionally generates only the winning (SQLite) dialect in R1; this draft is the
-- single source for the Postgres path if R2 switches DB. Authority: data/01 §1.x.4 Postgres草案.
--
-- This file is documentation/reserved-only. Do not wire it into the SQLite migration runner.

CREATE TYPE identity_type AS ENUM (
  'standard_full_time', 'dispatch', 'part_time', 'new_employment',
  'domestic_service', 'construction_labor', 'individual_employee',
  'intern', 'retired_rehired', 'contractor', 'de_facto_no_contract'
);
CREATE TYPE dispute_subtype AS ENUM (
  'social_ins_waiver_invalid','social_ins_arrears','social_ins_underpaid_base',
  'social_ins_intermittent','social_ins_proxy','social_ins_uninsured_injury',
  'social_ins_cross_period','social_ins_base_dispute'
);
CREATE TYPE coverage_tier AS ENUM ('make_deep', 'make_usable');
CREATE TYPE case_status AS ENUM ('draft', 'diagnosed', 'confirmed', 'frozen', 'disputed');
CREATE TYPE fact_category AS ENUM (
  'relation_qualification','wage','time_period','termination_reason',
  'work_injury','discrimination','other'
);
CREATE TYPE fact_status AS ENUM ('pending', 'confirmed', 'disputed', 'deprecated');
CREATE TYPE fact_source AS ENUM ('user_input', 'ai_inferred', 'rule_engine');
CREATE TYPE coverage_tag AS ENUM ('exact', 'approximate', 'boundary', 'unknown');
CREATE TYPE evidence_type AS ENUM (
  'documentary_contract','audio_video','digital_communication',
  'witness_statement','scene_photo_video','third_party_data','appraisal'
);
CREATE TYPE evidence_status AS ENUM (
  'uploaded','parsed','scored','verified','disputed','quarantined'
);
CREATE TYPE claim_type AS ENUM (
  'economic_compensation','economic_damage','double_wage_no_contract',
  'overtime_pay','malicious_arrears_surcharge','work_injury_benefit','other'
);
CREATE TYPE claim_status AS ENUM ('draft', 'finalized', 'granted', 'denied', 'withdrawn');

CREATE TABLE "case" (
  id                   UUID PRIMARY KEY,
  identity_type        identity_type NOT NULL,
  dispute_subtype      dispute_subtype NOT NULL,
  dispute_category     VARCHAR(16) NOT NULL CHECK (dispute_category ~ '^LD-[0-9]{2}-[0-9]{2}$'),
  coverage_tier        coverage_tier NOT NULL DEFAULT 'make_usable',
  case_occurred_at     DATE NOT NULL,
  province             VARCHAR(2) NOT NULL CHECK (province ~ '^[0-9]{2}$'),
  city                 VARCHAR(4) NOT NULL CHECK (city ~ '^[0-9]{4}$'),
  region_code          VARCHAR(6) NULL CHECK (region_code IS NULL OR region_code ~ '^[0-9]{6}$'),
  kb_version_hash      CHAR(64) NOT NULL CHECK (kb_version_hash ~ '^[0-9a-f]{64}$'),
  kb_version_label     VARCHAR(64) NOT NULL,
  status               case_status NOT NULL DEFAULT 'draft',
  group_id             UUID NULL,
  dialogue_template_id VARCHAR(64) NULL,
  created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  frozen_at            TIMESTAMPTZ NULL,
  CONSTRAINT frozen_consistent CHECK (
    (status = 'frozen' AND frozen_at IS NOT NULL) OR
    (status <> 'frozen' AND frozen_at IS NULL)
  ),
  CONSTRAINT region_prefix_consistent CHECK (
    region_code IS NULL OR (left(region_code, 4) = city AND left(region_code, 2) = province)
  )
);
CREATE INDEX idx_case_status ON "case"(status);
CREATE INDEX idx_case_group ON "case"(group_id) WHERE group_id IS NOT NULL;
CREATE INDEX idx_case_region ON "case"(province, city);

CREATE TABLE fact (
  id                   UUID PRIMARY KEY,
  case_id              UUID NOT NULL REFERENCES "case"(id) ON DELETE RESTRICT,
  content              TEXT NOT NULL,
  category             fact_category NOT NULL,
  status               fact_status NOT NULL DEFAULT 'pending',
  source               fact_source NOT NULL,
  confidence           REAL CHECK (confidence >= 0 AND confidence <= 1),
  coverage_tag         coverage_tag NOT NULL,
  evidence_refs        UUID[] NOT NULL DEFAULT '{}',
  group_id             UUID NULL,
  contributor_id       UUID NULL,
  authorization_chain  JSONB NULL,
  created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT ai_must_have_confidence CHECK (source <> 'ai_inferred' OR confidence IS NOT NULL)
);
CREATE INDEX idx_fact_case ON fact(case_id, status);

CREATE TABLE evidence (
  id                UUID PRIMARY KEY,
  case_id           UUID NOT NULL REFERENCES "case"(id) ON DELETE RESTRICT,
  evidence_type     evidence_type NOT NULL,
  file_path         TEXT NOT NULL,
  file_hash         CHAR(64) NOT NULL CHECK (file_hash ~ '^[0-9a-f]{64}$'),
  mime_type         VARCHAR(64) NOT NULL,
  byte_size         BIGINT NOT NULL CHECK (byte_size > 0 AND byte_size <= 104857600),
  effective_score   REAL NOT NULL CHECK (effective_score BETWEEN 0 AND 1),
  score_breakdown   JSONB NOT NULL,
  status            evidence_status NOT NULL DEFAULT 'uploaded',
  high_sensitivity  BOOLEAN NOT NULL DEFAULT FALSE,
  collected_at      TIMESTAMPTZ NULL,
  device_id         VARCHAR(128) NULL,
  gps_coords        JSONB NULL,
  chain_membership  TEXT[] NOT NULL DEFAULT '{}',
  created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT score_breakdown_keys CHECK (
    score_breakdown ?& array['source','timing','completeness','relevance','authenticity']
  ),
  CONSTRAINT case_file_unique UNIQUE (case_id, file_hash)
);
CREATE INDEX idx_evidence_case_type ON evidence(case_id, evidence_type);
CREATE INDEX idx_evidence_chain ON evidence USING GIN (chain_membership);
CREATE INDEX idx_evidence_quarantine ON evidence(case_id) WHERE status = 'quarantined';

CREATE TABLE claim (
  id                          UUID PRIMARY KEY,
  case_id                     UUID NOT NULL REFERENCES "case"(id) ON DELETE RESTRICT,
  claim_type                  claim_type NOT NULL,
  amount_pre_tax              NUMERIC(14, 2) NULL CHECK (amount_pre_tax IS NULL OR amount_pre_tax >= 0),
  amount_post_tax             NUMERIC(14, 2) NULL CHECK (amount_post_tax IS NULL OR amount_post_tax >= 0),
  calculation_breakdown       JSONB NOT NULL,
  status                      claim_status NOT NULL DEFAULT 'draft',
  law_refs                    UUID[] NOT NULL DEFAULT '{}',
  fact_refs                   UUID[] NOT NULL DEFAULT '{}',
  withdraw_inv10_confirmed    BOOLEAN NOT NULL DEFAULT FALSE,
  created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT inv10_withdraw_consistent CHECK (status <> 'withdrawn' OR withdraw_inv10_confirmed = TRUE)
);
CREATE INDEX idx_claim_case ON claim(case_id, status);

CREATE TABLE law_ref (
  id                UUID PRIMARY KEY,
  stable_id         TEXT NOT NULL CHECK (stable_id LIKE 'law:%'),
  content_hash      CHAR(64) NOT NULL CHECK (content_hash ~ '^[0-9a-f]{64}$'),
  kb_version_label  VARCHAR(64) NOT NULL,
  title             TEXT NOT NULL,
  version_date      DATE NOT NULL,
  article           VARCHAR(32) NOT NULL,
  body_snapshot     TEXT NOT NULL,
  created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (stable_id, content_hash)
);
CREATE TABLE case_law_ref (
  case_id       UUID NOT NULL REFERENCES "case"(id) ON DELETE RESTRICT,
  law_ref_id    UUID NOT NULL REFERENCES law_ref(id),
  used_in       VARCHAR(32) NOT NULL CHECK (used_in IN ('diagnosis','document','calculation')),
  PRIMARY KEY (case_id, law_ref_id, used_in)
);
CREATE INDEX idx_law_ref_stable ON law_ref(stable_id);
CREATE INDEX idx_case_law_ref_case ON case_law_ref(case_id);

CREATE TABLE "group" (
  id                  UUID PRIMARY KEY,
  case_id             UUID NOT NULL REFERENCES "case"(id) ON DELETE RESTRICT,
  initiator_id        UUID NOT NULL,
  inv10_confirmed_at  TIMESTAMPTZ NOT NULL,
  created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE group_contributor (
  group_id            UUID NOT NULL REFERENCES "group"(id) ON DELETE CASCADE,
  contributor_id      UUID NOT NULL,
  authorization_chain JSONB NOT NULL,
  joined_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (group_id, contributor_id)
);
CREATE INDEX idx_group_case ON "group"(case_id);
