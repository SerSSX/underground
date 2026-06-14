-- P1 #75 (review fix): backfill. 0026 added orders.email_sent DEFAULT false, which on an
-- existing DB marks every historical paid order as un-emailed — a first-deploy "email blast".
-- Mark all pre-existing orders as already handled; only orders created after this runs (default
-- false) enter the outbox. (On a fresh DB the orders table is empty → this is a no-op.)
UPDATE orders SET email_sent = true WHERE email_sent = false;
