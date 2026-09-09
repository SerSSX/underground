-- PAY-5, contract step of expand/contract. 0045 added the mirror of the provider's own view of
-- the organiser's account; the code has since stopped reading these columns (the GDPR erasure
-- statement was the last reader). Now they go.
--
-- This is deliberately a SEPARATE migration from 0045, run after the code move, because the
-- order is the whole point: drop them in the same step and any deploy that briefly runs the old
-- code against the new schema fails on a missing column.
--
-- Why drop rather than keep: under direct charges the organiser is the merchant of record and
-- their money never passes through us, so their IBAN/SWIFT is data we have no purpose for. Data
-- we do not hold cannot leak, and holding other people's bank details without a reason is
-- exactly what data minimisation forbids.
ALTER TABLE users
    DROP COLUMN payout_name,
    DROP COLUMN payout_iban,
    DROP COLUMN payout_swift;
