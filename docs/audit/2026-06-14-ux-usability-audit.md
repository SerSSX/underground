# UX usability test + product-design audit — underground

**Date:** 2026-06-14 · **Method:** 13-agent run — 6 persona task-walkthroughs (usability) + 6 PD heuristic dimensions, synthesized.
**Overall UX score: 58/100 (late-beta).**

> Verdict: strong bones and brand (type-safe stack, dark-theme tokens, co-located i18n, client-side QR, transparent escrow/GDPR policy), but UX maturity is held back by one systemic problem — **the web client is a half-finished clone of the more complete Android app, and the backend ships capabilities the UI never exposes.** Core happy paths (browse, save, buy a test ticket) feel polished; every journey touching money, age, safety, refunds, or moderation hits a missing affordance, a silent failure, or a cross-platform inconsistency.

## Scorecard (PD dimensions, /10)
| Dimension | Score |
|---|---|
| Trust, safety & money UX | **4.2** |
| Accessibility | **5.2** |
| Information architecture & navigation | 6.2 |
| Content, microcopy & i18n | 6.2 |
| Visual design & consistency | 6.5 |
| Conversion funnel & onboarding | 6.5 |

## Usability (persona / SUS)
| Persona | SUS | Outcome |
|---|---|---|
| First-time visitor (browse→buy) | 64 | 4 ok / 2 partial / 1 blocked (no web onboarding) |
| Returning attendee | 58 | 4 ok / **1 fail (no ticket return on web)** |
| Organizer (Android+web) | 55 | 3 ok / 2 partial / 1 fail |
| Privacy user (export/delete) | 58 | 6 ok / 1 partial |
| Safety (report + moderate + dispute) | **42** | 6 partial / 3 fail |
| Age-gate buyer | **38** | 1 ok / 2 partial / 1 fail / 1 blocked |

## Critical issues
1. **Web missing ticket return / dispute / transfer** (web/L) — `/refunds` policy says "Мої квитки → Повернути", but `web/app/me/tickets` has no such button; the API + Android have it. Broken promise + parity gap.
2. **Age-gating shown but not legible client-side** (all/M) — the `age_rating` pill is decorative; clients don't pre-check DOB or explain, so a user hits an opaque backend 409. *(Note: the backend DOES enforce age at checkout/reserve/transfer — P1 #77. The gap is purely client UX: pre-check + explain + one-tap path to set DOB.)*
3. **Admin moderation queue has no enforcement** (admin/L) — only status transitions (open→reviewed→resolved); no hide/delete/suspend/freeze/escalate, no context, no audit trail.

## High
- No post-registration onboarding on web (interests/city) — Android has it; web lands on empty "Для тебе".
- Organizer analytics endpoint exists (`organizerAnalytics`) but no web call site — verify & wire (or remove).
- `register` discards `?next=` (login honors it) → new-buyer funnel breaks.
- No checkout/escrow transparency: buy()→payTest()→redirect, no order summary, no escrow disclosure, no success state.
- Web report uses bare `window.prompt()`; Android can't report reviews.
- No password reset anywhere.
- Destructive actions (GDPR delete) use native prompt/alert, no accessible confirmation/success.
- Financial/KYC jargon shown as raw enums (held/clearing/frozen/paid_out) with no human labels/tooltips.
- Accessibility: no `:focus-visible`, color-only status badges, native modals.

## Quick wins (S, high-impact)
- `register` honor `?next=` + propagate through login↔register.
- `:focus-visible` outlines + `a:hover/:focus` in web globals.css.
- Text/icon prefixes on status pills (not color-only).
- Client-side validation on register/login.
- Wire `?new={ticketId}` to highlight the just-bought ticket.
- Success toasts (save/review/export/KYC).
- Remove "Шаблон — потребує юр-рев'ю" banners from legal pages (move to dated footer).
- Inline age explainer near Buy + on the age-rating selector.
- Fix Android KYC status showing "—" (ensure `loadKyc()` populates the flow).
- admin table `scope="col"`, form `role="alert"`; bump `--muted` contrast + 44px tap targets.

## Structural bets (L)
- **Web↔Android parity as a release gate**: ticket return/dispute/transfer, web onboarding, review reporting.
- **Legible money pipeline**: pre-purchase order summary + escrow disclosure, post-purchase confirmation w/ clearing date, buyer-visible escrow status on tickets, organizer payouts dashboard with estimated dates.
- **Real age-verification UX**: surface DOB requirement up front, client pre-check both platforms, one-tap recover-from-blocked-purchase.
- **Proper moderation console**: actionable queue (hide/delete/suspend/freeze/escalate), context, severity, escalation, audit trail, link reports↔disputed orders.
- **Wire analytics + promo end-to-end**: daily-sales + per-promo redemption views, promo max-use/expiry controls, fix edit-form capacity bug.
- **Shared design-system + content layer**: CSS type scale + semantic classes (kill inline styles), extracted tokens (admin/Android hardcode hexes), full i18n for legal + admin (UA-only today) + Android string extraction, financial-terms glossary as tooltips.

## Correction vs the raw audit
The audit twice claims age-gating "isn't enforced" / "no backend validation" — that's inaccurate: the backend enforces age at checkout, reserve, and transfer (P1 #77, gated). The real, valid finding is the **client-side legibility** gap (no pre-check/explanation → opaque 409).
