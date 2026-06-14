# Structural bets round — 2026-06-14

Follow-up to the UX usability audit (`2026-06-14-ux-usability-audit.md`) and the UX-critical
round (#83–87). The user selected **UX structural bets (L) + adversarial review + quick-wins (S)**.
This round closes the highest-value cross-surface gaps and flags the genuinely-large items with
effort estimates so they can be scheduled rather than half-built.

## Shipped

### 1. Password reset (was a hard release blocker)
Email/password accounts had **no recovery path** — a forgotten password locked a user out forever.

- **Backend** (`api/src/passwords.rs`, migration `0028_password_resets`): `POST /api/auth/forgot-password`
  and `POST /api/auth/reset-password`. Only the **SHA-256 hash** of the token is stored; the raw token
  travels by email and is never persisted. Tokens are **single-use** (claimed atomically with `used_at`)
  and **1-hour** expiry. `forgot-password` **always returns 204** (no account enumeration). Completing a
  reset invalidates the user's other outstanding tokens. Email transport is the same env-gated SendGrid
  follow-up as order confirmations (logged in dev).
- **Web**: `/forgot-password`, `/reset-password?token=…` pages + "Forgot password?" on `/login`.
- **Android**: "Forgot password?" on the auth screen requests a link; the reset itself completes on web.

### 2. Dispute-from-ticket parity (web ↔ Android)
Buyers could only dispute via deep paths; the ticket DTO didn't expose the order.

- **Backend** (`api/src/tickets.rs`): `Ticket` now carries `order_id` + `escrow_status` via a
  `LEFT JOIN orders`. Free reservations get `null`/`null` (nothing to dispute).
- **Web** (`/me/tickets`) and **Android** (`ETicketScreen`): a "Dispute this order" action appears only
  while escrow is `held|clearing`; shows "under review" once `frozen`.

### 3. Moderation audit-trail
Privileged admin actions left no accountable record.

- **Backend** (`api/src/audit.rs`, migration `0029_admin_actions`): every admin mutation
  (user block/role/verify, event moderation, escrow actions, post/review deletion, KYC decisions,
  payouts, report resolutions) writes an append-only `admin_actions` row (actor, action, target, JSON
  detail). `admin_id` is `ON DELETE SET NULL` so the trail outlives a removed admin. `GET /api/admin/actions`.
- **Admin**: new "Журнал дій" console page.

### 4. Android review-report parity
Android could report events but not individual reviews (web could do both). Added a per-review report
affordance in `ReviewsSection`.

**Quality bar:** backend re-gated green — `cargo test`, `cargo llvm-cov --fail-under-lines 100`
(100% line coverage, 0 missed), clippy `-D warnings`, rustfmt clean. web `tsc` 0, admin `tsc` 0,
Android `assembleDebug` SUCCESSFUL. Migrations now 0001–0029.

## Deferred (genuinely Large — flagged with estimates, not half-built)

| Item | Why deferred | Estimate |
|------|--------------|----------|
| **Shared design-system layer** (tokens/components across web + admin + Android) | The three surfaces each re-declare colors, spacing, and component styles. Unifying is a cross-repo refactor with high regression surface and **no functional gap** — purely consistency/velocity. Best done as its own milestone with visual regression coverage. | **L — ~2–3 wks** (1 wk design tokens + primitives per surface, 1–2 wks migrating existing screens + visual QA) |
| **Full admin-panel i18n (UA/EN)** | The admin console is an internal staff tool (UA-only today). Public surfaces (web, legal) are already UA/EN. Lower release-criticality than user-facing parity. ~40 strings across ~8 pages. | **M–L — ~1 wk** (extract strings + LangProvider parity with web's `lib/i18n`) |
| **Full Android UI i18n** | ~226 strings, 37 files, no `strings.xml` foundation yet. Web is the worldwide entry point; Android users are UA-first for now. | **L — ~1.5–2 wks** |

## Still ahead of launch (unchanged from the release plan)
P2 hardening (rate-limit, structured observability, CI/CD, prod Docker, backups); deploy the API
container with migrations 0020–0029; live Lemon Squeezy sandbox smoke; legal-counsel review of the
published policies.
