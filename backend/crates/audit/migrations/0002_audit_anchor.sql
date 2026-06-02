-- data/04 §4.6.4 — daily anchor table (singular `audit_anchor`) in the independent audit.sqlite.
-- R1a done criterion: ots_receipt IS NOT NULL AND ots_anchored_at IS NOT NULL.
CREATE TABLE audit_anchor (
  date               TEXT PRIMARY KEY,            -- 'YYYY-MM-DD'
  last_record_hash   TEXT NOT NULL CHECK (length(last_record_hash) = 64),
  record_count       INTEGER NOT NULL CHECK (record_count >= 0),
  daily_sha256       TEXT NOT NULL CHECK (length(daily_sha256) = 64),
  ots_receipt        BLOB NULL,                   -- OpenTimestamps .ots receipt bytes (R1a writes via mock/CLI)
  ots_anchored_at    TEXT NULL,                   -- ISO 8601; R1a required when anchored
  github_commit_sha  TEXT NULL CHECK (github_commit_sha IS NULL OR length(github_commit_sha) = 40),
  github_anchored_at TEXT NULL,
  schema_version     INTEGER NOT NULL DEFAULT 1
);
