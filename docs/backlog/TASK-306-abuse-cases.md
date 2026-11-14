# TASK-306 — Abuse cases for person-to-person blocking

Status: written BEFORE implementation, by an identity that will not implement it (rules §2 control 1, §3).
Scope: `docs/backlog/TASK-306-user-blocking.md` (S1–S8). Code read at `6b71351` (branch `claude/underground-mvp`):
`api/src/{messages,social,posts,notifications,tickets,orders,eventsocial,organizer,reviews,reports,gdpr,routes,push,email,main,auth,error,text,ratelimit,staff,reminders,users,admin}.rs`,
`api/migrations/0053_user_blocks.sql`, `web/app/u/[userId]/page.tsx`.

`users.blocked` is the ADMIN ban. Nothing in this file means it unless a row says "admin-banned".

## Conventions

- `A` = the blocker, `B` = the blocked person, `C` = an unrelated third user, `X` = an admin (`role = 'admin'`). All
  active (not admin-banned, not deleted) unless the row says so.
- "A blocks B" = `POST /api/users/<B>/block` with A's Bearer token answered 204, so a `user_blocks (A, B)` row exists.
- `P_A` = a post authored by A (any `kind`); `P_B`, `P_C` likewise. `E_A` = a free, published, future event organised by
  A; `E_B` likewise. `T_B` = an active ticket owned by B for some free event of C.
- "alert" = a `notifications` row. "visible to A" = present in `GET /api/me/notifications` as A; "counted for A" =
  included in `GET /api/me/unread`.
- A verdict names the HTTP status and the database state a test asserts. "No row" means no new row in the table the
  action writes (`user_blocks`, `messages`, `follows`, `post_likes`, `post_comments`, `notifications`).
- "Concurrent" rows release all requests from one barrier and repeat 50 times; one violation fails the row.
- The 403 refusals of S2, S3 and S5 tell B that a block exists between them. That is an accepted, deliberate
  disclosure (the owner's reading); rows below only check that nothing discloses MORE than that.

## Decisions taken here (each is a verdict some row asserts; flip one and those rows change)

- **D1** The 1001st block answers **409** (the API's status for a cap, as `ensure_recipient_has_room`), writes no row
  and touches no follow. The cap is checked before anything is deleted.
- **D2** `DELETE /api/users/:id/block`: self → 422. When the caller has a row for that id → 204 and the row is removed,
  whatever the target's state now (admin-banned included), so a row that counts toward the cap can always be removed.
  No row and the id is unknown, deleted or admin-banned → 404 (S1). No row and the id is an active user → 204.
- **D3** A repeated block keeps the first `created_at` (the "when" in the list does not move).
- **D4** `GET /api/me/blocks` returns every row that counts toward the cap, admin-banned targets included, all of them
  (at most 1000, no truncation). Deleted targets have no rows (S8).
- **D5** Block and unblock share one rate bucket per caller, `block:{user_id}`, `BLOCK_LIMIT_PER_MINUTE` (default 30)
  per 60 s; above it → 429 and no state change.
- **D6** Every S2/S3/S5 refusal is 403 with ONE direction-neutral body, the same bytes whichever side placed the block.
  It never names who blocked whom.
- **D7** S4 "nothing B does creates an alert for A" includes the `event_match` fan-out when B publishes an event: A
  gets no `event_match` alert for an event organised by B while A blocks B.
- **D8** Alerts from B are hidden from A at read time (list, unread count, push selection), not deleted, so they come
  back after an unblock. Exception: an alert from B still waiting for push (`pushed_at IS NULL`) when A blocks B is
  never pushed, not even after an unblock.
- **D9** From this task on, a `booking` alert stores the booker as `actor_id` (today `notify_booking` stores none), so
  S4 can hide it. Booking alerts created before the deploy have no actor and stay visible (accepted gap, see
  spec_conflicts in the hand-back).
- **D10** `comments_count` in any `PostView` served to A equals the number of comments A's comment list for that post
  returns (hide in list → hide in count). `likes_count` is unchanged: there is no list of likers.
- **D11** A write that S2, S3 or S5 refuses never commits after the block on that pair committed: the block and those
  writes serialise on the pair.

## 1. Block endpoints (S1)

| id | actor and action (endpoint, input) | state before | verdict | why |
|---|---|---|---|---|
| AB1 | A: `POST /api/users/<B>/block`, no body | follows A→B and B→A exist | ALLOW 204, empty body; one `user_blocks (A,B)` row; both follows rows gone; `GET /api/users/<B>` as A → `is_blocked: true` | S1, S3 |
| AB2 | A: same POST again | AB1 | ALLOW 204; still one row; `created_at` unchanged | S1 idempotent, D3 |
| AB3 | A: `DELETE /api/users/<B>/block` | AB1 | ALLOW 204; row gone; `is_blocked: false`; no follows row recreated | S1 |
| AB4 | A: same DELETE again | no row, B active | ALLOW 204 | S1 idempotent, D2 |
| AB5 | A: `POST /api/users/<A>/block` and `DELETE /api/users/<A>/block` | — | REJECT 422 each; no row; never 500 (the table's `CHECK` must not be what refuses) | S1 |
| AB6 | A: POST block on a random v4 UUID; on the nil UUID `00000000-0000-0000-0000-000000000000` | no such user | REJECT 404; no row | S1 |
| AB7 | A: POST block on B' who erased their account (`DELETE /api/me`) | B' `deleted_at` set | REJECT 404; no row | S1 |
| AB8 | A: POST block on B'' admin-banned (`users.blocked = true`) | — | REJECT 404; no row | S1 |
| AB9 | A: DELETE block on B, B admin-banned AFTER A blocked B | row (A,B) exists | ALLOW 204; row removed | D2: a row on the cap must stay removable |
| AB10 | A: DELETE block on a random UUID | no row | REJECT 404 | S1, D2 |
| AB11 | A: POST block on `/api/users/not-a-uuid/block`, `/api/users/123/block`, `/api/users/<B>x/block`, `/api/users/..%2F..%2Fadmin/block` | — | REJECT 400 (404 where the router matches nothing); never 2xx or 5xx; no row | typed `Path<Uuid>`, path traversal |
| AB12 | A: POST block on `/api/users/%00/block` and `/api/users/<B>%00/block` | — | REJECT 422 (router-wide NUL guard); no row | null bytes |
| AB13 | A: POST block with B's id upper-case, without hyphens, braced `%7B…%7D`, and as `urn:uuid:<B>` | AB1 | ALLOW 204 each (or 400 for a form the parser refuses); never a second row: exactly one `(A,B)` | encoding aliases of one id |
| AB14 | anonymous: `POST` and `DELETE /api/users/<B>/block`, `GET /api/me/blocks` | — | REJECT 401 each; no row | auth |
| AB15 | A's token variants: expired `exp`; HS256 under another secret; `alg: none`; `iat` before `tokens_valid_after` (after a password change) | — | REJECT 401 each, on all three endpoints; no row | forged / expired / revoked token |
| AB16 | A admin-banned, or A erased, using a token minted before | — | REJECT 401; no row | `CurrentUser` gate |
| AB17 | A: POST block on B with JSON body `{"blocker_id":"<C>","blocked_id":"<D>","created_at":"2000-01-01T00:00:00Z"}` | — | ALLOW 204; the only new row is `(A,B)` with `created_at` ≈ now; no `(C,*)` or `(*,D)` row | mass assignment: the body is ignored |
| AB18 | A: POST block with `Content-Type: application/json` and a 3 MB body | — | REJECT 413; no row | oversized input |
| AB19 | C: `DELETE /api/users/<B>/block` | (A,B) exists, no (C,B) | ALLOW 204 (C's own no-op); `(A,B)` still exists | IDOR: a caller removes only their own rows |
| AB20 | C: `GET /api/me/blocks`, also with `?user_id=<A>` and `?blocker_id=<A>` | A has rows, C none | ALLOW 200 `[]` each time; never A's rows | IDOR on the list |
| AB21 | anyone: `GET /api/users/<A>/blocks`, `GET /api/users/<B>/block`, `GET /api/users/<B>/blocked` | (A,B) exists | REJECT 404 or 405; body contains no user id | no endpoint reads someone else's blocks |
| AB22 | A: `GET /api/me/blocks` | A blocked B (name, avatar) and B'' who was admin-banned later | ALLOW 200: one entry each with `id`, name, avatar, when; no email key; the name follows the display-name rule (a name repeating the email shows «Гість») | S1, D4 |
| AB23 | A: POST block on a new X1 | A has 999 rows | ALLOW 204; 1000 rows | cap boundary |
| AB24 | A: POST block on a new Y | A has 1000 rows; follows A↔Y exist | REJECT 409; 1000 rows; no row for Y; follows A↔Y still exist | D1: cap before side effects |
| AB25 | A: POST block on B again | 1000 rows including B | ALLOW 204; 1000 rows | a repeat is not a new block |
| AB26 | A: DELETE one, then POST block on a new Y | 1000 rows | ALLOW 204, 204; 1000 rows | cap frees on unblock |
| AB27 | A: two concurrent POSTs for new X and Y | 999 rows | exactly one 204 and one 409 per round; 1000 rows after each round | cap under concurrency |
| AB28 | A: 20 concurrent POSTs for 20 new targets | 990 rows | exactly ten 204 and ten 409; 1000 rows | cap under concurrency |
| AB29 | A: two concurrent POSTs for B | no row | both 204; exactly one row; no 500 (primary-key conflict handled) | idempotent under concurrency |
| AB30 | A: concurrent POST and DELETE for B | no row | both 2xx, no 500; after the round either (row AND `is_blocked: true` AND no follows A↔B) or (no row AND `is_blocked: false`) | no half state |
| AB31 | A: 31 alternating block/unblock calls on B within 60 s | — | calls 1–30 → 204; call 31 → 429 and the state after call 30 is unchanged | D5 toggling |
| AB32 | A: as AB31, each request with a different `X-Forwarded-For` / `X-Real-IP` | — | call 31 → 429 | bucket keyed on the user, not on a header |
| AB33 | A: `GET /api/me/blocks` | 1000 rows | ALLOW 200 with all 1000 entries | D4, no silent truncation |
| AB34 | B after A blocks B: `GET /api/me`; `POST /api/messages` to C; C: `GET /api/users/<B>` | AB1 | ALLOW 200 / 201 / 200; B's `users.blocked` still false | a personal block is not the admin ban |
| AB35 | X bans B (`PATCH /api/admin/users/<B>` blocked=true), then unbans | A blocks B | after the unban `(A,B)` still exists and B→A message → 403 | the admin ban does not clear a personal block |
| AB36 | A: DELETE block on B | A blocks B; X banned B | ALLOW 204; B stays admin-banned; B's token → 401 | unblock does not unban |
| AB37 | A blocks admin X; X: `DELETE /api/admin/posts/<P_A>`; X: `GET /api/admin/reports` | A reported C | ALLOW 204 for the block; X's delete → 204; the queue still lists A's report | moderation is not subject to a block |

## 2. Messages (S2)

| id | actor and action | state before | verdict | why |
|---|---|---|---|---|
| AB38 | B: `POST /api/messages {"recipient_id":"<A>","text":"hi"}` | A blocks B | REJECT 403 (D6 body); no `messages` row | S2 |
| AB39 | A: `POST /api/messages {"recipient_id":"<B>","text":"hi"}` | A blocks B | REJECT 403; no row | S2 either way |
| AB40 | B: as AB38 with `recipient_id` upper-case and without hyphens | A blocks B | REJECT 403 (or 422 if the form does not parse); never 201 | encoding alias |
| AB41 | A: `GET /api/messages/<B>` | 3 messages A↔B before the block | ALLOW 200 with the 3 | S2 history kept |
| AB42 | B: `GET /api/messages/<A>` | as AB41 | ALLOW 200 with the 3 | S2 history kept |
| AB43 | A: `GET /api/me/conversations` | A has threads with B and C | ALLOW 200; no entry with `peer_id` B; C's entry present | S2 |
| AB44 | B: `GET /api/me/conversations` | as AB43 | ALLOW 200; entry with `peer_id` A present | S2 hides only on the blocker's side |
| AB45 | A: `GET /api/me/unread` | B sent 2 unread before the block, C sent 1 | `messages: 1` | S2: list hidden → count hidden |
| AB46 | B: `GET /api/me/unread` | A sent B 1 unread before the block | `messages: 1` | only the blocker's count is filtered |
| AB47 | A unblocks B; A: conversations and unread | after AB43, AB45 | B's entry back; `messages: 3` | hidden, not deleted |
| AB48 | A blocks B and B blocks A; A unblocks B; then A→B and B→A sends | — | REJECT 403 both (B's block stands); A's conversations show B again; B's hide A | mutual block: each row is its own |
| AB49 | B: 20 concurrent sends to A while A's block request runs | no block | the count of B→A messages read right after the block's 204 equals the count after every send has answered; every send after that → 403 | D11 race |
| AB50 | B erases the account and signs up again with the same email (new id B2); B2 messages A | A blocked B | ALLOW 201 | a block binds an account id; accepted residual (A blocks B2) |

## 3. Follows (S3)

| id | actor and action | state before | verdict | why |
|---|---|---|---|---|
| AB51 | A blocks B | A→B and B→A follows | both rows gone; `GET /api/users/<A>/followers` and `/following` (public) omit B, and B's omit A; `followers_count` / `following_count` on both profiles equal their list lengths | S3; list ↔ count |
| AB52 | B: `POST /api/users/<A>/follow`, also with A's id upper-case | A blocks B | REJECT 403; no `follows` row; no alert for A | S3, S4 |
| AB53 | A: `POST /api/users/<B>/follow` | A blocks B | REJECT 403; no row | S3 either way |
| AB54 | B: `DELETE /api/users/<A>/follow` | A blocks B | ALLOW 204 (no-op) | unfollow reaches nobody |
| AB55 | A unblocks B | AB51 | no follows row in either direction | unblock does not restore follows |
| AB56 | A blocks B, unblocks, B follows A (204, A gets `new_follower`), A blocks B again | — | B→A follow gone again; that `new_follower` alert not visible to A, not counted | follow re-created between blocks |
| AB57 | B: concurrent `POST /api/users/<A>/follow` while A's block runs | B does not follow A | after both answer: no follows row between A and B in either direction; no `new_follower` from B visible to or counted for A | D11 race; a surviving row would leak A's future events to B |
| AB58 | A: `GET /api/events/<E>/social`; B: same | A and B followed each other; both hold tickets for and saved published E; A blocks B | A's `friends_going` and `friends_interested` omit B; B's omit A; `going_count` and `interested_count` unchanged | mutual-follow friends lists |
| AB59 | B publishes an event in A's preferred category and city | A followed B before; A blocks B | no `organizer_event` and no `event_match` alert for A for that event; C with the same prefs gets `event_match` | S3, D7 |

## 4. Alerts and push (S4)

| id | actor and action | state before | verdict | why |
|---|---|---|---|---|
| AB60 | A: `GET /api/me/notifications`, `GET /api/me/unread` | before the block B followed A, liked and commented on `P_A` (3 alerts, actor B); C liked `P_A` (1); A blocks B | list holds only C's alert; `notifications` count = C's unread only | S4 hides alerts A already had |
| AB61 | A: `GET /api/me/notifications` | B offered A a place before the block (open offer, alert with claim link) and A has more than 100 newer alerts | B's offer alert absent from both parts of the list (newest 100 and open offers) | S4; the open-offer branch is a second path |
| AB62 | A: `GET /api/me/notifications` | 100 newest alerts are from B, 50 older from C; A blocks B | ALLOW 200 with C's 50 | hidden rows must not use up the limit |
| AB63 | B: `POST /api/events/<E_A>/reserve {"quantity":2}` | A blocks B | ALLOW 201 (S7); no `booking` alert for A from this booking | S4 booking, S7 |
| AB64 | B buys a place on a paid E_A (`POST /api/events/<E_A>/checkout`, then `/api/orders/:id/pay-test` or the webhook) | A blocks B | order paid and ticket minted; no `booking` alert for A | the second booking path (`orders::fulfill`) |
| AB65 | B: `POST /api/tickets/<T_B>/transfer {"email":"<A's email>"}` | A blocks B | ALLOW 202 `{"status":"sent"}`, byte-identical to a transfer to an unknown email; a pending `ticket_gifts` row; no alert for A | S6 |
| AB66 | B: `POST /api/tickets/<T_B>/gift {"email":"<A's email>"}` | A blocks B | ALLOW 201 with the same fields as a gift to an unknown email; no alert for A | S6, the other offer endpoint |
| AB67 | B: as AB65 with `"  A@EXAMPLE.COM "` (spacing and case variant of A's email) | A blocks B | ALLOW 202; no alert for A | the email resolves to A after normalisation |
| AB68 | B: `POST /api/tickets/<T_B>/transfer {"user_id":"<A>"}` | A blocks B | REJECT 422; no offer, no alert | there is no offer-by-id path |
| AB69 | B: transfer to A's email concurrent with A's block | no block | any `place_offer` alert from B that exists afterwards is not visible to or counted for A, and the push selection never returns it | race; read-time hiding (D8) |
| AB70 | B: reserve on E_A concurrent with A's block | no block | any `booking` alert from B created in the race carries `actor_id` B and is not visible to or counted for A | race; D9 |
| AB71 | push worker's selection `push::pending` | an alert from B to A with `pushed_at IS NULL`, A has a device token; A blocks B | no row for that alert on the next run, nor after A unblocks B | D8 queued alerts |
| AB72 | `push::pending` | an alert to A whose actor A currently blocks, inserted with `pushed_at IS NULL` after the block | no row for it | D8 push filter at send time |
| AB73 | A unblocks B; A reads list and unread | after AB60 | B's 3 alerts back in the list; unread ones counted again | D8 hidden, not deleted |
| AB74 | the email outbox `email::pending_notifications` | alerts from B of kinds `new_follower`, `post_like`, `post_comment`, `place_offer`, `booking` | no row for any of them | B-originated alerts never go out by email |
| AB75 | B publishes an event matching A's prefs | A blocks B, does not follow B | no `event_match` alert for A for it; C with the same prefs gets one | D7 |
| AB76 | A: list | a `booking` alert created before the deploy for B's booking (`actor_id` NULL) | ALLOW: still visible | D9 accepted gap |
| AB77 | A: transfers `T_A` to B's email; A books E_B | A blocks B | B gets a `place_offer` alert and E_B's booking alert | ALLOW: S4 protects the blocker only; these are A's own acts |
| AB78 | A transfers to B's email; B transfers to A's email | A blocks B and B blocks A | both 202; neither gets an alert | mutual block |
| AB79 | reminder sweeps (`event_reminder`, `review_prompt`, `selling_fast`) | A holds a ticket for E_B and saved E_B; A blocks B | ALLOW: A still gets these alerts for E_B | triggered by A's own booking or save, not by B |

## 5. Posts, likes and comments (S5)

| id | actor and action | state before | verdict | why |
|---|---|---|---|---|
| AB80 | B: `POST /api/posts/<P_A>/like` | A blocks B | REJECT 403; no `post_likes` row; no alert | S5 |
| AB81 | A: `POST /api/posts/<P_B>/like` | A blocks B | REJECT 403; no row | S5 either way |
| AB82 | B: `POST /api/posts/<P_A>/comments {"text":"x"}` | A blocks B | REJECT 403; no `post_comments` row; no alert | S5 |
| AB83 | A: `POST /api/posts/<P_B>/comments {"text":"x"}` | A blocks B | REJECT 403; no row | S5 either way |
| AB84 | B: like and comment on P_A of each kind: `text`, `event_share`, `review` (auto post) | A blocks B | REJECT 403 for each | every post kind |
| AB85 | B: `DELETE /api/posts/<P_A>/like` | B liked P_A before the block | ALLOW 204; the like is removed | unlike reaches nobody |
| AB86 | C and anonymous: `GET /api/posts/<P_A>/comments`; any viewer's `likes_count` of P_A | B liked and commented on P_A before the block | ALLOW: B's comment listed; B's like counted | the block deletes nothing (see spec_conflicts) |
| AB87 | A (signed in): `GET /api/posts/<P_A>/comments` | B and C commented before the block | HIDDEN from A: B's comments absent, C's present | S5 |
| AB88 | A: `GET /api/posts/<P_C>/comments` | B and C commented on C's post | HIDDEN from A: B's comments absent | S5 "any comment list A reads" |
| AB89 | anonymous: `GET /api/posts/<P_C>/comments` | as AB88 | ALLOW: all comments incl. B's | hiding is per viewer |
| AB90 | B: `GET /api/posts/<P_A>/comments` and `<P_C>` | A commented on both | ALLOW: A's comments listed | S5 hides only in the blocker's view |
| AB91 | A: `GET /api/posts/<P_C>/comments` | the 200 oldest comments on P_C are B's, then 10 by C | ALLOW 200 with C's 10 | hidden rows must not use up `LIMIT 200` |
| AB92 | A: any `PostView` of P_C (feed, `GET /api/users/<C>/posts`) | 3 comments by B, 2 by C | `comments_count: 2` for A; `3+2 = 5` for C and anonymous | D10 list ↔ count |
| AB93 | A: `GET /api/feed` | in A's interest category: B's `event_share` of E_B, B's review post of E_A, and C's post | HIDDEN from A: neither of B's posts; C's present | S5 feed |
| AB94 | A: `GET /api/feed` | 60 newest matching posts by B, 10 older by C | ALLOW 200 with C's 10 | hidden rows must not use up `LIMIT 50` |
| AB95 | B: `GET /api/feed` | A's `event_share` in B's interest category | ALLOW: present | S5 filters the blocker's feed only |
| AB96 | A: `GET /api/users/<B>/posts` | B has posts | ALLOW 200 with them | S7 profiles stay public |
| AB97 | B: likes on 10 posts of A, concurrent with A's block | no block | the count of B's likes on A's posts read right after the block's 204 equals the count after every like has answered; no like alert from B visible to A | D11 race |
| AB98 | B: comments on 10 posts of A, concurrent with A's block | no block | same as AB97 for comments | D11 race |

## 6. Place offers (S6) — beyond AB65–AB69

| id | actor and action | state before | verdict | why |
|---|---|---|---|---|
| AB99 | — (A blocks B) | B's pending offer of T_B to A's email | offer stays `pending`, ticket stays `pending_claim`; B: `DELETE /api/tickets/<T_B>/gift` → 200, ticket `active` | the block cancels nothing; B keeps control of B's ticket |
| AB100 | A: `POST /api/tickets/claim` with the token of B's offer (link got outside the app) | A blocks B | ALLOW 200; A owns the ticket | the block governs what reaches A, not what A chooses to accept |
| AB101 | B: `POST /api/tickets/claim` with a gift A sent to another address (link forwarded) | A blocks B | ALLOW 200; no alert for A | bearer link; no alert exists for a claim |
| AB102 | B: `GET /api/me/tickets` after AB65, compared with a transfer to an unknown email | — | same status (`pending_claim`) and fields | S6: B learns nothing more |

## 7. Kept on purpose (S7)

| id | actor and action | state before | verdict | why |
|---|---|---|---|---|
| AB103 | A: `GET /api/events/<E_A>/guests` | B holds 2 places on E_A; A blocks B | ALLOW 200: B listed with name and places | S7 door check |
| AB104 | A or A's staff: `POST /api/events/<E_A>/scan` with B's ticket QR | as AB103, event within check-in | ALLOW: valid, B's name shown | S7 door check |
| AB105 | B: `POST /api/reviews` for E_A (attended, after the start) | A blocks B | ALLOW 201; listed in `GET /api/events/<E_A>/reviews` for A and anyone; counted in A's `rating_avg` / `rating_count` | S7 reviews are neither hidden nor refused |
| AB106 | A: `POST /api/reviews` for E_B (attended) | A blocks B | ALLOW 201 | S7 |
| AB107 | B: `GET /api/users/<A>`, `/events`, `/posts`; A: `GET /api/users/<B>`, `/events` | A blocks B | ALLOW 200 each | S7 profiles and events public |
| AB108 | B: `POST /api/reports` on user A and on P_A; A: on user B | A blocks B | ALLOW 201 each; listed in `GET /api/admin/reports` | reporting must stay open both ways |
| AB109 | B (accepted staff on E_A before the block): `POST /api/events/<E_A>/scan`; A: `DELETE /api/events/<E_A>/staff/<B>` | A blocks B | ALLOW: B's scan works; A's removal → 204 and B's next scan → 403 | the block does not touch staff; the organiser removes staff |

## 8. Information leaks

| id | actor and action | state before | verdict | why |
|---|---|---|---|---|
| AB110 | B: `GET /api/users/<A>`; C (neither follows A nor blocked): same | A blocks B | the two JSON bodies have the same keys and values; `is_blocked: false`; no key that says B is blocked | S1 "no field says B is blocked by A" |
| AB111 | anonymous and a caller with an invalid token: `GET /api/users/<B>` | A blocks B | 200, `is_blocked: false` | S1 anonymous |
| AB112 | A: `GET /api/users/:id` for each id in A's `GET /api/me/blocks`, and for C | — | `is_blocked: true` for each listed id, `false` for C; B: `GET /api/users/<A>` → `false` | flag ↔ list; the flag is the caller's own block only |
| AB113 | the 403 bodies of AB38, AB39, AB52, AB53, AB80, AB81, AB82, AB83 | — | all one body text (D6); none contains A's id or name or a phrase meaning "blocked you" | nothing beyond the accepted 403 |
| AB114 | A: POST and DELETE block on B | B has / has not blocked A | 204 with an empty body in both states | blocking is not a probe for "did B block me" |
| AB115 | A: POST block on an active, an admin-banned, an erased and a random id | — | 404 exactly when `GET /api/users/<id>` is 404 | no new existence oracle |
| AB116 | the web server render of `/u/<B>` (`web/app/u/[userId]/page.tsx`) | — | its API fetch carries no `Authorization` header or cookie; the cached HTML never contains `"is_blocked":true` | a cached per-viewer flag would leak A's block list |
| AB117 | A with a Bearer token: `GET /api/users/<B>`, `/api/me/blocks`, `/api/posts/<P>/comments`, `/api/feed` | — | `Cache-Control` has neither `public` nor `s-maxage` | a shared cache must not store a per-viewer answer (RFC 9111 §3.5) |
| AB118 | B: `GET /api/me/export` | A blocks B | no entry, key or value about A's block of B | S8 lists blocks a person MADE |
| AB119 | web, B's chat with A after a 403 on send | A blocks B | the text shown is neutral; no string in any of the 8 locales used there means "you were blocked" | S1 "nothing ever tells B" |
| AB120 | web «Заблоковані» list | A blocked B whose name is `<img src=x onerror=alert(1)>` | the name renders as text; no script runs; no `img` element from the name | XSS in the new list |

## 9. Account erasure and export (S8)

| id | actor and action | state before | verdict | why |
|---|---|---|---|---|
| AB121 | A: `DELETE /api/me` | A blocked B and C; D blocked A | no `user_blocks` row with `blocker_id` A or `blocked_id` A; D's `GET /api/me/blocks` no longer lists A and D's cap count dropped by 1 | S8 both ways (users are anonymised, not deleted, so `ON DELETE CASCADE` never fires) |
| AB122 | B: `DELETE /api/me`; then A: list and POST block on a new Y | A had 1000 rows incl. B | A's list drops B; the new block → 204 | S8 frees the cap |
| AB123 | A: `GET /api/me/export` | A blocked B at t1 and C at t2; D blocked A | a blocks list with exactly two entries, each with the blocked id and its time, no email; nothing about D's block | S8 |
| AB124 | A: `GET /api/me/export` after unblocking C | — | only B's entry | export shows current blocks |

## Attack-class checklist (walked)

| class | result |
|---|---|
| Injection — SQL | N/A for input: the feature takes no text; ids arrive as `Uuid` and must be bound. The new filters are SQL fragments composed with `format!`/`concat!` of constants only, never of request values. |
| Injection — XSS | AB120 (names in the new list); the 403 text is server-fixed (AB113). |
| Injection — command, LDAP, template | N/A: no shell, directory or server-side template is touched. |
| Auth bypass — missing, expired, forged, replayed, revoked token | AB14, AB15, AB16; anonymous profile view AB111. Replay of a valid token inside its life is the account itself: N/A. |
| IDOR / privilege escalation | AB19, AB20, AB21 (someone else's rows); AB17 (blocker taken from the token); AB37 (admin powers unaffected); AB34–AB36 (personal block vs admin ban). |
| Input abuse — oversized, boundary, null bytes, unicode, malformed | AB18 (3 MB body), AB23–AB28 (999/1000/1001), AB6 (nil UUID), AB12 (NUL), AB11 (malformed id), AB13 and AB40 (alias encodings of one id), AB67 (email spacing and case). Unicode in names: names are cleaned at write by existing code; rendering covered by AB120. Deeply nested JSON: the block endpoints read no body (AB17), N/A. |
| Rate limiting / brute force | AB31, AB32 (toggling); AB115 (no existence oracle to brute force). Sends, follows and transfers keep their existing limits. |
| Header manipulation | AB32 (`X-Forwarded-For` does not reset the bucket); AB116, AB117 (caching of per-viewer answers). Host header and CORS: N/A, the new routes inherit the existing CORS layer and security headers unchanged. |
| Path traversal | N/A for files: the id is `Path<Uuid>`, never a file path; AB11 pins `..%2F` to 400/404. |
| Mass assignment / parameter pollution | AB17 (body ignored), AB20 (query params ignored), AB68 (`user_id` instead of `email`). |
| Resource leaks | N/A for handles: no new long-lived handle, cache or listener; rows per blocker are capped at 1000 (AB24); hidden alerts are existing rows, not new growth. |

## Companion pairs (frame-lock check)

| mechanism | companion | rows |
|---|---|---|
| A blocks B | B blocks A (mutual), and each unblock removes only its own row | AB48, AB78, AB114 |
| block | unblock (state restored or not: follows not, history and alerts yes) | AB3, AB47, AB55, AB73 |
| block refuses a write | the same write racing the block | AB49, AB57, AB69, AB70, AB97, AB98, AB30 |
| hide in list | hide in count (unread messages, unread alerts, `comments_count`, follower counts, block list vs cap) | AB45, AB60, AB92, AB51, AB22/AB33 |
| hide in list | hidden rows do not consume the page limit | AB62, AB91, AB94 |
| one endpoint | the other endpoint reaching the same person: comment vs message, gift vs transfer, paid vs free booking, open-offer branch vs newest-100 branch, `event_match` vs `organizer_event` | AB82, AB66, AB64, AB61, AB59 |
| one endpoint | the bulk or list endpoint (`/api/me/blocks`, conversations, feed, comment list, friends lists) | AB22, AB43, AB93, AB88, AB58 |
| in-app alert | push outbox and email outbox | AB71, AB72, AB74 |
| alert after the block | alert already held or queued before it | AB60, AB61, AB71, AB76 |
| id in the path | alias encodings of the same id, and an email instead of an id | AB13, AB40, AB52, AB67, AB68 |
| size cap (1000) | concurrency at the cap, repeat at the cap, list length at the cap | AB27, AB28, AB25, AB33 |
| time window (a block stands) | replay inside it: follow re-created between two blocks | AB56 |
| personal block | admin ban (`users.blocked`) and its undo | AB8, AB9, AB34, AB35, AB36 |
| refusal status | refusal body (direction-neutral) | AB113 |
| per-viewer field | shared cache and server render that drop the viewer | AB116, AB117 |
| account erasure | erasure of the other side; export of the other side | AB121, AB122, AB118, AB123 |
| a block binds an account | a new account of the same person | AB50 |
