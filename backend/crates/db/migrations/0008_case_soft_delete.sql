-- 0008_case_soft_delete.sql — soft-delete column for the case root aggregate.
-- backend/01 §1.2 DELETE /case/:id = soft delete with a 30-day recycle bin: instead of a hard
-- DELETE (FK-RESTRICT-blocked while children exist), a case is marked deleted by stamping
-- `deleted_at` (TIMESTAMPTZ-as-TEXT, RFC3339). Soft-deleted cases are excluded from GET /case list
-- and GET /case/:id (which then returns E_NOT_FOUND). Additive single-direction migration: no
-- down.sql, no DROP (backend/02 §2.13).
ALTER TABLE "case" ADD COLUMN deleted_at TEXT NULL;
CREATE INDEX idx_case_deleted_at ON "case"(deleted_at) WHERE deleted_at IS NOT NULL;
