-- data/04 §4.3 — independent audit.sqlite (D3). Table name `audit_log` is retained (data/04 §4.3
-- explicitly keeps the plural-free name but in the independent library; I8 ruled this non-conflicting
-- with D9 since audit_log is not a D9 first-class object). The main store must NOT define this table.
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;

CREATE TABLE audit_log (
  seq               INTEGER PRIMARY KEY AUTOINCREMENT,  -- monotonic, no gaps
  prev_hash         TEXT NOT NULL,                       -- previous record_hash, forms the chain
  record_hash       TEXT NOT NULL UNIQUE,                -- SHA-256(prev_hash || payload)
  who_kind          TEXT NOT NULL,
  who_payload       BLOB NOT NULL,                       -- ChaCha20-Poly1305 ciphertext of Subject JSON
  who_nonce         BLOB NOT NULL,                       -- 12-byte random nonce for who_payload (W5)
  when_ts           INTEGER NOT NULL,                    -- Unix epoch ms
  why               TEXT NOT NULL,                       -- AuditReason text (snake_case)
  what_payload      BLOB NOT NULL,                       -- ChaCha20-Poly1305 ciphertext of what JSON
  nonce             BLOB NOT NULL,                       -- 12-byte random nonce for what_payload (W5)
  schema_version    INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX idx_audit_when ON audit_log(when_ts);
CREATE INDEX idx_audit_why ON audit_log(why);

-- Append-only enforcement (data/04 §4.1 / §4.3): UPDATE / DELETE are rejected by triggers.
CREATE TRIGGER audit_no_update BEFORE UPDATE ON audit_log
BEGIN
  SELECT RAISE(ABORT, 'audit_log is append-only');
END;

CREATE TRIGGER audit_no_delete BEFORE DELETE ON audit_log
BEGIN
  SELECT RAISE(ABORT, 'audit_log is append-only');
END;
