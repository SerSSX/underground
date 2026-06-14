-- P1 #73: GDPR account erasure marker. We anonymize the user row in place (rather than
-- hard-deleting) because orders/tickets FK to users ON DELETE CASCADE — a hard delete would
-- destroy financial records that must be retained for tax/audit.
ALTER TABLE users ADD COLUMN deleted_at TIMESTAMPTZ;
