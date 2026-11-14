# TASK-306 — users can block other users

Status: ON DEV (2026-11-14, dffe72a, migration 53); prod waits for the owner's command. Lane: TASK, SENSITIVE (authorization between users). Brief mode: pro — the
owner's yes settles the go; what a block does is the common reading below, recorded here.

## Request (verbatim)

Question to the owner, 2026-11-14: «Чи робити блокування користувачів до релізу?» Answer: «Так, зробити».

## What a block does (orchestrator's reading, 2026-11-14)

A blocks B. Admin bans (`users.blocked`) stay a separate thing; this is a person-to-person block.

- S1 API: `POST /api/users/:id/block` and `DELETE /api/users/:id/block`, both idempotent, 204. Self → 422; an
  unknown, deleted or admin-banned id → 404. `GET /api/me/blocks` lists the people A blocked (id, name, avatar,
  when). The public profile gains `is_blocked` = the caller blocked this user (false for anonymous callers). Nothing
  ever tells B that A blocked them, and no field says "B is blocked by A". At most 1000 blocks per user.
- S2 Messages: no new message either way while the block stands (403). Both keep the history they had. A's
  conversation list hides B, and A's unread-message count does not count B.
- S3 Follows: blocking removes A→B and B→A; a new follow either way is refused (403) while the block stands.
- S4 Alerts: nothing B does creates an alert for A (follow, like, comment, place offer, booking). Alerts from B that
  A already had are hidden from A's list and unread count.
- S5 Posts: B cannot like or comment on A's posts, and A cannot on B's (403). A's feed and any comment list A reads
  leave out B's posts and comments.
- S6 Place offers: an offer from B to A's email goes on exactly as one to an unknown email: B sees 202 and a pending
  offer, A gets no alert.
- S7 Unchanged on purpose: profiles and events stay public; B can still book A's events and A's guest list still
  shows B (the door check needs it); reviews are neither hidden nor refused (a block must not let an organiser bury
  reviews). Barring a person from one's own events is a separate feature, not built.
- S8 Account deletion removes the person's blocks both ways; the data export lists the blocks a person made.
- Web: on a profile, «Заблокувати» / «Розблокувати» with a confirmation; a chat with a blocked person shows why
  the box is closed and an unblock button; a «Заблоковані» list in the profile with unblock. All 8 languages.
- Android: not in this task; the app ignores the new field. To do at the Android step.

## Abuse-case triage by the orchestrator, 2026-11-14

Table: `TASK-306-abuse-cases.md` (124 rows, written by a separate identity before the code).

- Assumptions: 409 at the cap with nothing changed (D1); one rate bucket `block:{user}` for block and unblock, 30 a
  minute (D5); a repeated block keeps its first time (D3); the list includes targets banned later (D4); the comment
  count matches the comment list (D10); 400 for a malformed id; a claim link still works between them (AB100, AB101);
  staff rights are untouched (AB109); a new account of the same person is not blocked (AB50): **all accepted**.
- Conflict 1, event alerts: **accepted** — no `event_match` or `organizer_event` alert to A for B's events; both
  kinds now store the organiser as `actor_id`, so older ones hide too.
- Conflict 2, booking alerts had no actor: **accepted** — new ones store the booker; older ones (DEV only) stay.
- Conflict 3, B's old comments stay under A's posts for others: **resolved by a new rule** — a post's author can
  delete any comment under it (`DELETE /api/posts/:id/comments/:comment_id`). API only for now: the web has no
  comment UI yet.
- Conflict 4, unblocking an admin-banned person: **accepted** (D2) — an existing block is always removable.

## Tests (690ef4e, written by a separate identity) — triage 2026-11-14

`api/tests/task306_blocks.rs`: 47 passed. Control-presence: 17 named controls plus 4 extra, all killed (notify_actor's
filter only after a dedicated test was added). Assumptions accepted: AB49 reads right after the 204; setup by SQL
inserts; AB66 compares field names; the AB57/AB97 race is stood in by a direct `notify_actor` call (read-time hiding
covers the rest). Conflict on AB22: the recorded rule (docs/decisions.md 2026-11-14, a typed name is shown) wins.

## Review and DEV (2026-11-14)

Review (separate identity): NO-GO on one MAJOR — the uk/de text said the blocked person "won't find out", while they
do see the 403s; now «Ми їй про це не повідомимо». Fixed in ce1e5e5 with three smaller ones (no push of an alert
written during the block after an unblock; erasure locks the account row first; the inbox list reloads after an
unblock). Closure check (separate identity): all four closed, two tests added (dffe72a), 49 pass. CI green on
dffe72a; DEV deploy smoke green. Browser on DEV, WebKit iPhone 13: `web/e2e/block-webkit.mjs` 15/15 (written by the
implementer, a flow check on top of the independent API suite), `web/e2e/chat-webkit.mjs` 14/14.
