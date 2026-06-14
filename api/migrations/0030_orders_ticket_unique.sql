-- Enforce the one-order-per-ticket invariant the code already assumes (dispute DTO LEFT JOINs
-- orders on ticket_id). A partial unique index allows many free reservations (ticket_id NULL)
-- while guaranteeing a minted ticket maps to at most one order — so the join can never fan out.
CREATE UNIQUE INDEX IF NOT EXISTS orders_ticket_id_uidx ON orders (ticket_id) WHERE ticket_id IS NOT NULL;
