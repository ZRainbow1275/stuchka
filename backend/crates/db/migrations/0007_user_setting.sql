-- 0007_user_setting.sql — auxiliary local-settings singleton (backend/04 §4.3/§4.4/§4.7;
-- cross-crate-reconciliation B-6 / D). NOT a D9 first-class object: added so crypto (I-8) and db
-- can persist the encryption/recovery state. SQLite fallback (data/01 §1.0.5):
--   UUID->TEXT(36); TIMESTAMPTZ->TEXT RFC3339; ENUM->TEXT+CHECK IN.
-- Singleton: the object keeps a v7-UUID id (D9), but the table holds at most one row. The
-- partial unique index on the constant 1 enforces "exactly one row" without hard-coding the id
-- value (a fixed-id CHECK would contradict the v7-UUID id). Application upsert is single-row.
CREATE TABLE user_setting (
    id                   TEXT PRIMARY KEY,
    fs_encryption_status TEXT NOT NULL DEFAULT 'unknown' CHECK (fs_encryption_status IN (
                           'enabled','disabled','disabled_with_ack','unknown')),
    recovery_method      TEXT NULL CHECK (recovery_method IS NULL OR recovery_method IN (
                           'keychain','bip39','usb_key')),
    bip39_check_hash     TEXT NULL,
    master_pwd_argon2    TEXT NULL,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
);

-- Singleton guard: at most one row may ever exist (single local-settings record).
CREATE UNIQUE INDEX idx_user_setting_singleton ON user_setting((1));
