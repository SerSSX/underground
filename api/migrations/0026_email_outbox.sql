-- P1 #75: transactional email. A per-order watermark so a paid order is emailed exactly
-- once (duplicate webhook deliveries are idempotent — the order is already email_sent).
ALTER TABLE orders ADD COLUMN email_sent BOOLEAN NOT NULL DEFAULT false;
