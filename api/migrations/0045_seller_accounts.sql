-- PAY-5: the organiser becomes the merchant of record for their own tickets. Money goes
-- straight to their connected account at the PSP (direct charges + application fee), so the
-- platform never holds buyers' money — and therefore no longer needs, or wants, to store their
-- bank details. What we keep instead is a MIRROR of the PSP's own view of that account.
--
-- Expand step of expand/contract: this migration only ADDS. The bank columns it replaces are
-- dropped in 0047, after the code has stopped reading them.
ALTER TABLE users
    ADD COLUMN psp_account_id       TEXT,
    ADD COLUMN psp_charges_enabled  BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN psp_payouts_enabled  BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN psp_requirements_due JSONB,
    ADD COLUMN psp_country          TEXT;

-- One connected account belongs to exactly one organiser. This is load-bearing, not tidiness:
-- the `account.updated` webhook finds its user BY this id, so a second user carrying the same
-- id would let one organiser's onboarding switch on another organiser's ability to take money.
-- Partial, because every user who has not onboarded yet holds NULL here.
CREATE UNIQUE INDEX users_psp_account_uidx ON users (psp_account_id) WHERE psp_account_id IS NOT NULL;
