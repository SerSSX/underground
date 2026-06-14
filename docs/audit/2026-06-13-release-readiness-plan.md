# Release-readiness plan — underground

**Date:** 2026-06-13
**Source:** 30-agent orchestrated verification + design pass over the 2026-06-14 product audit, plus a 5-lens gap scan (ops/observability, security, data/money, release-engineering, compliance/privacy) and a synthesis.
**Verdict going in:** production-ready = **NO** (6.1/10). Core journeys are strong (81/81 live checks); the blockers are security, money-correctness, ops and compliance.

Every backend task below must pass the standing gate before merge:
`cargo test` green · `cargo llvm-cov --ignore-filename-regex 'src/main\.rs' --fail-under-lines 100` · `cargo clippy --all-targets -- -D warnings` · `cargo fmt --check`.

---

## Verification corrections (audit claims re-checked against live code)

- **All 12 known findings confirmed still present** at the cited locations.
- **discover/feed indexes:** the audit's "missing indexes" claim is partly wrong — `post_likes` PK and `post_comments_post_idx` already serve those joins. The real cost is **correlated subqueries**; the fix is pre-aggregating social counts via grouped joins (moved to P3, not a launch blocker).
- **live-payment:** not a logic bug — the webhook + HMAC + idempotent fulfill are correctly implemented. It's an **operational/safety gap**: live mode can be switched on with incomplete creds and the end-to-end loop is unverified against real Lemon Squeezy.
- **blocked-jwt / escrow-release / promo-timing / cors / email-case / free-ticket-dedup** — all confirmed exactly as reported, with exact file:line and a designed fix.

---

## Phased plan

### P0 — Launch blockers (critical security & money-correctness) · ✅ DONE (2026-06-13)

**Status: all 6 shipped and gated** — `cargo test` green · `cargo llvm-cov --ignore-filename-regex 'src/main\.rs' --fail-under-lines 100` exit 0 (100% lines) · clippy `-D warnings` clean · rustfmt clean. No new migrations (schema unchanged). A 6-agent adversarial review followed; it surfaced and we fixed: (a) **refund idempotency** — repeated admin refund no longer double-frees the seat / double-returns the promo use (seat + promo credit now keyed off the ticket-return `rows_affected`); (b) **CORS hardening** — malformed `CORS_ALLOWED_ORIGINS` entries are now rejected at startup by `Config::validate` (fail-fast) and a dropped-origin warning was added, removing the silent-filter footgun. One decision recorded: `fulfill` deliberately does **not** re-check promo `expires_at` (the discount is locked at checkout; only `max_uses` must be atomic) — re-checking would reject an already-paid live order.

Nothing ships until every P0 passes the gate.

| Task | Effort | What |
|---|---|---|
| `blocked-jwt-central-check` | S | `CurrentUser` extractor (auth.rs:139-148): after `decode_user_id`, run `EXISTS(SELECT 1 FROM users WHERE id=$1 AND blocked=false)` → 401 if not active. Single chokepoint for ~50 handlers. No migration. Test `blocked_user_loses_jwt_access`. |
| `escrow-release-dispute-guard` | S | `admin.rs escrow_action`: add `force: Option<bool>`; `release` only `held|clearing → cleared`; `frozen` → cleared **only if** `force:true`, else 409. Update the freeze→release test. |
| `promo-timing-at-payment` | M | `apply_promo` becomes a non-mutating SELECT; `used_count` increments atomically in `fulfill` (guarded, idempotent); admin refund decrements it. Reverses denial-of-promo. |
| `https-and-secret-hardening` | M | `config.rs`: fail-fast in prod if `JWT_SECRET` is the dev default or <32 bytes; require live LS creds when mode=live. Document mandatory TLS reverse proxy + HSTS + HTTP→HTTPS. |
| `cors-env-allowlist` | S | Replace `CorsLayer::permissive()` with env-driven `CORS_ALLOWED_ORIGINS` allow-list (explicit methods/headers, `allow_credentials`). Empty → permissive dev fallback (WARN). Depends on `https-and-secret-hardening`. |
| `live-payment-readiness-guard` | M | Reject `mode=live` save unless all four LS fields set (422); checkout in live mode with no variant → 409 (no orphan order); `GET /api/admin/payment-readiness`. Pre-launch sandbox smoke checklist. |

### P1 — Must-fix before public launch · ✅ BACKEND DONE (2026-06-13)

**Status: all 12 P1 backend items shipped and gated** — `cargo test` green · `cargo llvm-cov --ignore-filename-regex 'src/main\.rs' --fail-under-lines 100` exit 0 (100% lines + functions, 3231 lines) · clippy `-D warnings` clean · rustfmt clean. Migrations 0020–0026 (sequential, self-healing where indexes meet dirty data). Decisions taken: age-gating **enforced** (DOB + per-event rating, gated at checkout/reserve); GDPR delete **anonymizes PII in place** (financial rows retained, de-identified — orders FK is ON DELETE CASCADE so a hard delete would destroy them); email = **SendGrid** via an env-gated outbox seam (order-confirmation now; refund/payout reuse the seam); KYC = gate + bank-detail validation + provider-agnostic seam (no provider selected yet). The **4 legal policies are written** in `docs/legal/` (privacy, terms, refund, abuse) — pending counsel review.

**Frontend brought to readiness (2026-06-14) — all P1 features wired into the clients:**
- **web** (tsc clean): legal pages `/privacy /terms /refunds /abuse` + footer links; `/account` (DOB, GDPR export/delete with re-auth, organizer KYC form); `age_rating` selector in EventForm; age badge + Report (event & per-review) on event detail; Account link in TopNav.
- **admin** (tsc clean): escrow Release sends `force:true` on frozen (with confirm); Users page shows `kyc_status` + Verify/Reject KYC; new **Reports** moderation queue (`/reports`).
- **Android** (`assembleDebug` BUILD SUCCESSFUL): Settings screen DOB + GDPR export/delete + KYC form; `age_rating` dropdown in CreateEvent; age badge + Report dialog on event detail; data layer (Retrofit endpoints + DTOs).
- Backend `AdminUser.kyc_status` added for the admin UI (coverage-neutral; re-gated 100%).

(Original estimate retained for reference: ~9–13 weeks; delivered the backend in this pass.)

| Task | Effort | What |
|---|---|---|
| `free-ticket-dedup` | S | Migration: partial unique index `tickets (event_id, owner_id) WHERE status='active'` + pre-check in `reserve`. One active ticket per user/event; re-reserve allowed after return. |
| `email-case-insensitive` | S | `normalize_email` (trim+lowercase) on register/login/transfer; migration lowercases existing + unique index on `lower(email)`. |
| `gdpr-account-deletion-export` | L | `DELETE /api/me` (re-auth, anonymize PII / cascade) + `GET /api/me/export` (JSON). Must reconcile with 7-yr financial retention → anonymize, don't hard-delete orders. Depends on `data-retention-policy`. |
| `legal-policy-docs` | M | Privacy Policy, ToS, Refund, Abuse policies; hosted on web at `/privacy /terms /refunds /abuse`; linked from settings/checkout/event. Refund text must match escrow semantics. |
| `transactional-email` | L | Provider (SES/SendGrid, EU region) behind a trait; order-confirm / refund / payout / password-reset emails; `email_sent` dedupe flag; triggered in webhook fulfill + admin refund/payout. |
| `abuse-report-mechanism` | M | `reports` table + `POST /api/reports` + `GET /api/admin/reports` + moderation actions; Report affordances in web/admin. |
| `age-gating` | M | **Decision required:** enforce (capture DOB, gate checkout/reserve, hide restricted from anon) or descope (drop unused `age_rating` column). |
| `kyc-payout-eligibility` | L | Require name/ID/tax + validated IBAN/SWIFT before first payout; `create_payout` checks verified; KYC provider (Sumsub/Onfido) above a threshold; audit trail. |
| `jwt-algorithm-pinning` | S | Explicit `Header::new(HS256)` + `Validation::new(HS256)`; reject alg-substitution. |
| `payout-rounding-numeric` | S | All-NUMERIC fee math in `create_payout` (no float); cent-exact vs client Decimal. |
| `admin-like-escape` | S | Escape `\ % _` + `ESCAPE '\'` in admin users/events/telemetry search (no wildcard enumeration). |
| `webhook-isolation-level` | S | SERIALIZABLE/REPEATABLE READ on `order_paid` fulfillment tx; concurrency test proves single ticket. |

### P2 — Hardening & ops · ~6–9 weeks (the abuse-prevention + indexing + CI + backup subset is launch-gating in practice)

`rate-limiting` (M, DB-backed on auth+checkout, 429) · `money-query-indexes` (S, escrow/status + dashboard, verify with EXPLAIN) · `audit-and-transaction-ledger` (M, every admin/money mutation) · `readiness-probe-pool-config` (M, `/readiness` + prod pool + request timeouts) · `graceful-shutdown` (M, server + workers) · `observability-stack` (L, JSON logs + correlation IDs + Prometheus `/metrics` + error reporting) · `ci-cd-pipeline` (M, GitHub Actions: fmt/clippy/test/100%-cov/build/smoke + image publish) · `prod-docker-stack-envs` (L, web/admin Dockerfiles + stage/prod compose + env separation; depends on CI) · `backup-dr` (M) · `deployment-runbook` (M, safe migration + rollback; depends on prod-docker) · `password-policy` (S) · `security-headers` (S, X-Frame-Options/CSP/HSTS/X-Content-Type-Options; depends on cors).

### P3 — Post-launch polish · ~6–8 weeks

`fcm-real-delivery` (L, FCM v1 + retry/dead-letter) · `pagination` (M, limit/offset on all lists) · `city-filter-case-insensitive` (S) · `discover-feed-perf` (M, pre-aggregate social counts via grouped joins) · `data-retention-policy` (M, worker) · `feature-flags` (M) · `api-compat-versioning` (M; depends on CI) · `web-accessibility-audit` (M, WCAG 2.1 AA).

---

## Critical path (launch gate)

`blocked-jwt-central-check → escrow-release-dispute-guard → promo-timing-at-payment → https-and-secret-hardening → cors-env-allowlist → live-payment-readiness-guard → free-ticket-dedup → email-case-insensitive → data-retention-policy → gdpr-account-deletion-export → legal-policy-docs → transactional-email → payout-rounding-numeric → rate-limiting → money-query-indexes → ci-cd-pipeline → prod-docker-stack-envs → deployment-runbook → backup-dr`

## Effort rollup

- **P0:** ~1.5–2 weeks · **P1:** ~9–13 weeks · **P2:** ~6–9 weeks · **P3:** ~6–8 weeks.
- **Full:** ≈ 23–32 engineer-weeks. **Safe public launch** = P0 + P1 + the abuse/indexing/CI/backup subset of P2 ≈ **15–20 weeks**.

## Top risks

1. **Migration-number collision:** ~6 designs each target `0020`. Renumber sequentially in merge order or `sqlx::migrate!` fails; second merger rebases.
2. **Behavioral breaks in lockstep:** frozen-release now 409 (admin UI must send `force:true`), live save now 422 on missing creds, case-insensitive email + password policy change auth UX, CORS/rate-limit can block existing clients — web/admin/Android must update together.
3. **Dirty-prod data blocks two unique indexes** (`lower(email)`, active-ticket partial): dedupe step must run before/with those migrations.
4. **100% coverage gate is unforgiving:** the rate-limiting AppState change touches ~30 test files (compile-break risk); new error paths need forced-failure tests.
5. **GDPR-deletion vs 7-yr financial retention:** anonymize PII while keeping financial rows; depends on retention policy being defined first.
6. **Real-money correctness can't be proven in CI** (no network in the gate): the manual staging LS sandbox smoke is the single biggest residual risk — do not skip.
7. **Ops gaps are launch-blocking in practice:** no graceful shutdown + no backups + no `/readiness` means a routine deploy or DB incident can corrupt escrow state or lose money data.

## Definition of done (release)

- All P0 + P1 complete and gated green (test · 100% cov · clippy · fmt).
- Migrations apply cleanly from scratch, sequential, no duplicate numbers; dirty-prod cleanup documented.
- Rate limiting live (429); money/dashboard indexes verified by EXPLAIN.
- `/readiness` validates DB; prod pool + timeouts + graceful shutdown; JSON logs + correlation IDs + `/metrics` + error reporting; audit + transaction ledgers.
- CI/CD gates every PR and publishes versioned images; web/admin prod Dockerfiles + env separation; deployment + rollback runbook; tested backup/DR.
- Documented end-to-end LS sandbox smoke executed on staging: payment → webhook → ticket → escrow held → sweep → cleared → payout net of fee.
