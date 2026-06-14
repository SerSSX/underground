-- P1 #77: age-gating. Buyers get a date of birth; events already carry age_rating
-- (since 0001) but it was never enforced. NULL dob = unknown (treated as not old enough
-- for any restricted event).
ALTER TABLE users ADD COLUMN dob DATE;
