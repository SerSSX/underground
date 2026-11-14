# TASK-306 — users can block other users

Status: IN PROGRESS (2026-11-14). Lane: TASK, SENSITIVE (authorization between users). Brief mode: pro — the
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
