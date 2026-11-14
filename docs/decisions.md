
## 2026-11-12 — GitHub repository made public (CI minutes)

Context: the SerSSX account's 2000 free Actions minutes for November ran out (shared with msgview); every
Underground and msgview CI job was refused before start. Options put to the owner: a $10 Actions spending limit,
make the Underground repo public (unlimited free minutes, incl. arm64 runners), or wait until 1 Dec.
Owner's answer (verbatim): «Зробити репо Underground публічним».
Before the switch: all 7 remote branches' full history scanned for committed secret files (.env, keys, keystores,
credentials) and secret patterns (Stripe/AWS/GitHub/Google/SendGrid/private keys/JWTs) — none found.
Consequence: the code, docs and history are readable by anyone; secrets stay out of the repo as before (values
live only on the box / in GitHub secrets).

## 2026-11-13 — OpenStreetMap data under ODbL accepted

Context: TASK-303 stores OSM-derived cities (names, coordinates) learned from Photon and publishes them via
/api/cities; accurate time zones from coordinates need tzf-rs, whose bundled boundary data is ODbL-1.0.
Options put to the owner: accept ODbL (attribution "© OpenStreetMap contributors"; a publicly shared database
derived from OSM stays under ODbL) or do not store OSM data (no learned cities, country-table zones only).
Owner's answer (verbatim): «Так, приймаємо ODbL (Recommended)».
Consequences: tzf-rs (MIT code + ODbL data) may be adopted; OSM attribution shown site-wide; the learned city
list stays open (it is public through /api/cities already).

## 2026-11-13 — TASK-304: three upload tests changed for WebP output (owner approval, §5)

Behaviour change asked by the owner (TASK-304 item 4): every upload is re-encoded to WebP.
- `api/tests/uploads.rs::upload_then_serve` — was: URL ends `.png`, served bytes equal the input; now: URL ends
  `.webp`, served bytes start with the RIFF/WEBP magic; why: uploads are no longer stored as sent.
- `api/tests/uploads.rs::upload_accepts_jpeg_and_webp` — was: fake bytes named `.jpg` accepted; now: real JPEG and
  WebP images, URL ends `.webp`; why: fake bytes are now rejected by content sniffing (a stricter control).
- `api/tests/uploads_errors.rs` — the fake PNG payload replaced by a real PNG; assertions unchanged.
Owner's answer (verbatim): «Так, погоджую (Recommended)».

## 2026-11-13 — TASK-304: outbound mail budget (orchestrator reading, reversible)

The review found that real mail (reset links, login codes) can be sent to any address. Reading taken without
asking (cheap to change, env-configurable): a global daily send budget (`MAIL_DAILY_BUDGET`, default 80, below
Resend's free 100/day) and a per-recipient cap; when the budget is spent the API refuses new mails with an
honest "mail is temporarily unavailable" instead of claiming it was sent. Per-address limits stay strict: an
attacker can delay a victim's codes/reset by spamming that address, a known trade-off versus mail-bombing; a
password account can still sign in with its password.

Updated after review round 3 (still the orchestrator's reading, reversible): a per-IP cap of 10 mail slots per
24 h (`MAIL_DAILY_PER_IP`), so one IP can no longer spend the whole budget; the per-address cap is 5 per rolling
hour (was 10 per day), so a targeted lockout ends about an hour after the attacker stops. All counted before the
account lookup, identically for every address. **Residual, for the owner:** an attacker with many IPs (8+ at the
default caps) can still spend the daily budget, which stops reset and code mail for everyone until slots free up
(rolling 24 h), or keep one address delayed for as long as the attack lasts. Closing that needs a CAPTCHA, a paid
mail plan with a higher budget, or both: a product and money decision.

## 2026-11-13 — TASK-304: residual mail risk accepted for DEV, CAPTCHA before prod

Question put to the owner: the many-IP residual above (budget exhaustion / delaying one address); options were
accept for DEV with a CAPTCHA (Cloudflare Turnstile, free) before prod, a CAPTCHA now, or accept permanently.
Owner's answer (verbatim): «Прийняти для DEV, капча до проду (Recommended)».
Consequence: Turnstile on the code-request and forgot-password forms is a prerequisite of Underground's first
prod release (tracked in docs/STATE.md); the Turnstile site key/secret are created by the owner.

## 2026-11-13 — TASK-304: upload capacity residual accepted for DEV, CAPTCHA on sign-up before prod

Residual after review round 5: three accounts together can keep the 3-place upload waiting room busy (each holds
one place up to the 60 s body deadline and retries), so others get 503 "too many uploads". Same capacity question
as the mail residual; closed by a CAPTCHA (Turnstile) on sign-up.
Owner's answer (verbatim): «Так, як з поштою (Recommended)».
Consequence: Turnstile goes on sign-up as well as the code-request and forgot-password forms, before Underground's
first prod release.

## 2026-11-13 — TASK-305: transfer refusal no longer reveals age (existing test change, owner approval §5)

`api/tests/age_gating.rs:157` — was: a transfer to an under-age recipient expects the specific 409 «отримувач не
відповідає віковому обмеженню події»; now: it expects the shared transfer refusal (same answer as an unknown
recipient), the transfer is still refused; why: the specific message told the sender the recipient is a minor
(security review TASK-305 F2).
Owner's answer (verbatim): «Так, погоджую (Recommended)».

## 2026-11-13 — TASK-305: reviews only from guests (orchestrator reading, reversible)

Security review F9: an organiser could review their own event and anyone could review an event they never booked.
Reading taken: a review needs a ticket (any non-cancelled place) for that event, and the organiser cannot review
their own event; reviews of cancelled events do not count. Easy to relax if the owner wants open reviews.

## 2026-11-14 — TASK-305: transfer-by-email becomes an offer the recipient accepts

Review of 15926f2 found transfer still an oracle through success vs refusal (two own events with different age
ratings → whether an account exists and whether its holder is an adult) and that it drops a stranger's ticket into
anyone's list. Options put to the owner: make it like the gift (recipient accepts; age checked at acceptance;
sender always sees "sent"), remove transfer-by-email, or leave it on DEV until prod.
Owner's answer (verbatim): «Як подарунок (Recommended)».

## 2026-11-14 — TASK-305: readings taken without asking (reversible)

- Placeholder names: «Гість» is shown only when the stored name is blank or literally «Гість»; legacy DEV rows whose
  name was auto-derived from the email (before TASK-302) are backfilled to «Гість» once. A name a person typed is
  always shown, even if it matches their email's local part. (Review finding: the generated-column rule hid chosen
  names such as "KyivJazz" for kyivjazz@….)
- Door check-in opens 6 hours before an event starts; earlier scans answer "too early" with the start date. A
  checked-in place of an event that has not ended stays under "Upcoming".
- In-app alerts: a place offered to you (gift or transfer) and a new booking on your event (organiser) create an
  alert; new chat messages show as an unread badge on Chats, not as alerts.

## 2026-11-14 — TASK-307: no ticket offer to one's own email; 3 test assertions changed

Owner reported that a gift to oneself "works" but nothing arrives, and asked to forbid it. Gift and transfer to the
sender's own address now answer 422 «квиток уже ваш — надішліть його іншій людині» (overrides TASK-305 R2 for the
sender's own address only). Assertion changes put to the owner (was · now · why):
1. `api/tests/tickets_actions.rs` transfer_validation: transfer to own email 202 → 422, ticket stays the sender's, active.
2. `api/tests/task305_round2.rs` r2_transfer_answer_never_depends_on_the_email: own email removed from the
   same-answer list, asserted separately as 422; the property holds for every other address.
3. `api/tests/task305_round2.rs` ux7_gift_offers_and_chats: gift to own email 201 → 422.
Why, all three: the owner's decision above.
Owner's answer (verbatim): «Так, погоджую».

## 2026-11-14 — Project closed; DEV removed from the server

Owner (verbatim gist): «закриваю проект ... повертати цю ідею немає сенсу, бо ніяким чином я її не просуну ... зносю з
сервера бекенд і всі контейнери, які пов'язані з андерграундом, щоб все ... працювало, крім андерграунду».
Why: a two-sided, city-bound event product with no distribution channel and no marketing budget. The code stays on
GitHub (SerSSX/underground); the DEV stand, its database and images are deleted from 65.108.87.247 (DEV data only;
there never was a prod).
