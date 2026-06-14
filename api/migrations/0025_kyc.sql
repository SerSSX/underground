-- P1 #78: lightweight KYC before organizer payouts (AML/fraud). kyc_status gates payouts;
-- bank details are collected for settlement. (Identity-provider integration plugs into the
-- 'pending' -> 'verified' transition later; for now an admin reviews and verifies.)
ALTER TABLE users ADD COLUMN kyc_status   TEXT NOT NULL DEFAULT 'none'; -- none|pending|verified|rejected
ALTER TABLE users ADD COLUMN payout_name  TEXT;
ALTER TABLE users ADD COLUMN payout_iban  TEXT;
ALTER TABLE users ADD COLUMN payout_swift TEXT;
