-- Postgres DRAFT DDL — user_setting auxiliary singleton (backend/04 §4.3/§4.4/§4.7;
-- cross-crate-reconciliation B-6 / D). NOT executed by the R1 runner: R1 ships SQLite only
-- (L0-03). dialect.rs intentionally generates only the SQLite dialect in R1; this draft is the
-- single source for the Postgres path if R2 switches DB. Documentation/reserved-only.
--
-- Mirrors crates/db/migrations/0007_user_setting.sql. The native ENUM types declared here are the
-- Postgres counterparts of the SQLite TEXT+CHECK columns.

CREATE TYPE fs_encryption_status AS ENUM (
  'enabled', 'disabled', 'disabled_with_ack', 'unknown'
);
CREATE TYPE recovery_method AS ENUM ('keychain', 'bip39', 'usb_key');

CREATE TABLE user_setting (
  id                   UUID PRIMARY KEY,
  fs_encryption_status fs_encryption_status NOT NULL DEFAULT 'unknown',
  recovery_method      recovery_method NULL,
  bip39_check_hash     TEXT NULL,
  master_pwd_argon2    TEXT NULL,
  created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Singleton guard: at most one row may ever exist (single local-settings record).
CREATE UNIQUE INDEX idx_user_setting_singleton ON user_setting ((TRUE));
