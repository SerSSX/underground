# TASK-117 — Tracking plan + воронка покупки + K-factor атрибуція

**Status:** IN PROGRESS (funnel API DONE, verified)

> **Реалізовано (verified `cargo test`):** `GET /api/admin/analytics/funnel` (`telemetry::analytics_funnel`) —
> впорядкована воронка (event_view→signup→ticket→gift_created→ticket_claimed) + **K-factor** (claims/gifts) за 7 днів;
> нові емісії подій `event_view` (get_event), `gift_created`/`ticket_claimed` (tickets) поверх наявного telemetry-seam.
> 2 тести ([SEC] admin-only 403; funnel рахує loop, K=1.0) + 0 регресій; повний суіт 235 passed / 0 failed.
> **Залишок:** повний ~20-подієвий tracking plan (client-side емісії view/tier_select/checkout_start/share/invite),
> share_token-атрибуція джерел (разом із TASK-112 deep-links), north-star дашборд у admin UI.
**Created:** 2026-06-29  **Updated:** 2026-06-29

## Raw Request
> «TASK-117 Tracking plan + воронка покупки + K-factor атрибуція» (ініціатива INIT-ANALYTICS, Phase 1, орієнтовна оцінка MEDIUM). Фокус: Event-tracking spec (~20 подій: view/detail_open/tier_select/checkout_start/paid/ticket_claimed/share/invite/guest_signup/rsvp/scan), серверна воронка view→paid→attend, K-factor через share_token, north-star дашборд в admin. Декомпозуй (tracking seam, funnel, dashboard).

## Refined Description
**Scope:**
- Канонічний **Tracking Plan v1** (~20 подій) з фіксованою property-схемою й ЄДИНИМ point-of-emit на кожну подію; розширює наявний `telemetry::log` seam (`api/src/telemetry.rs`) — нуль нових екранів, нуль нових event-таблиць (реюз `telemetry_events`, 0016).
- Серверна **purchase-воронка** `view → detail_open → tier_select → checkout_start → paid → ticket_claimed → attend(scan)` з drop% між кроками; endpoint `GET /api/admin/telemetry/funnel` + timeseries + сегментація по `source/channel`.
- **K-factor атрибуція** через `share_token` (парна атрибуція `*_sent ↔ *_accepted`, `attributed_signup`): `share/invite/gift → claim → guest_signup → re_share`; `GET /api/admin/telemetry/kfactor`.
- **North-star дашборд** в `admin/app/telemetry`: NSM (квитки/активного/28д), воронка з drop%, K-factor, channel-breakdown, дзеркально до фінансового timeseries.
- Інструментація точок-емісії у клієнтах: Android (`android/.../ui/**`) і public web (`web/app/**`) шлють події через наявний `POST /api/telemetry`; серверні події (signup/paid/scan/claim) емітяться інлайн у відповідних хендлерах.

**Non-goals:**
- Жодного нового платіжного коду; `api/src/escrow.rs` (provider-neutral NoopProvider) НЕ чіпається; гроші не рухаються.
- Не CRM / не lifecycle-тригери (це INIT-LIFECYCLE-TRIGGERS) і не A/B-платформа (INIT-EXPERIMENTS).
- Не створення gift/share-token seam (постачається INIT-GIFTLINK/INIT-DEEPLINKS); тут лише **читання** `share_token/token_nonce` як атрибуційного ключа.
- Не нові екрани продукту; дашборд — суто admin read-only.
- Без raw-PII/raw-token у props (лише id/nonce-hash).

**Impacted UCs:** new UC required (UC-«Product analytics & funnel», UC-«K-factor attribution») — фіксується у `docs/spec.md` через TASK-101 (spec reconciliation, INIT-ROADMAP-SPEC) ПЕРЕД кодом (rules §0/§2.2). Дотичні існуючі: перегляд події, checkout/paid, QR-scan, guest-claim.

**Impacted BR/WF:**
- BR: north-star = квитки (paid+free/RSVP) на активного/28д; вхідна метрика росту K-factor ≥ 0.5; пороги релізу share→claim ≥ 30%, guest→signup ≥ 40% (roadmap §9.1–§9.2) — фіксуються у spec через TASK-101.
- WF: purchase-funnel `view→detail_open→tier_select→checkout_start→paid→ticket_claimed→attend`; viral-loop `share/invite/gift→claim→guest_signup→re_share`.

**Dependencies:**
- **TASK-101** (spec reconciliation / INIT-ROADMAP-SPEC) — MUST land first: фіксує north-star/K-factor/пороги і нові UC у `docs/spec.md` (rules §0/§2.2).
- **TASK-006/008** (tickets, payments-testmode) — джерела серверних подій `paid`, `checkout_start`.
- **TASK-007** (qr-scan) — джерело події `scan/attend`.
- INIT-GIFTLINK / INIT-DEEPLINKS — постачають `share_token`/`token_nonce` (атрибуційний ключ). До їх готовності K-factor рахується на клієнтському `?ref`/share-CTA seam; funnel не блокується.
- TASK-016 (public web client) — host для web-точок емісії.
- Наявні seam: `api/src/telemetry.rs::log`, `POST /api/telemetry`, `GET /api/admin/telemetry`, `telemetry_events` (0016), `CurrentAdmin` (authz).

**Risks / Open Questions:**
- Ідентифікація анонімів для крос-крокової воронки: потрібен `anon_id` (клієнтський) до логіну + merge у `user_id` при signup. Ризик подвійного рахунку — мітигуємо детермінованим `anon_id` у props і `attributed_signup`-мостом.
- `attend` = успішний `scan_result{verdict=ok}`; для free/RSVP без сканера attend може бути недооціненим — задокументувати як known-limitation.
- Cardinality props (`event_id`, `channel`) — індекс лише `(name, created_at)`; важкі агрегати можуть бути повільні → GIN на `props` за потреби (окремий leaf INFRA).
- Публічний `POST /api/telemetry` — anti-abuse (rate-limit) наразі відсутній у repo → SEC-вимога у SDD, реалізується в API-leaf.
- Пороги/визначення NSM — не вигадувати; беруться з TASK-101/spec.

## Estimation
**Level:** HIGH (initial MEDIUM переоцінено — §12.5 flag+confirm)
**Justification:** Задача перетинає ≥4 шари (API tracking-seam + funnel-агрегація, Android емісія, Web емісія, Admin дашборд), два нові UC + spec-reconciliation dependency, дві окремі WF (purchase-funnel і viral-loop/K-factor), і послідовні фази (event-contract → server-emit → client-emit → funnel-API → dashboard). Це перевищує LOW-межу одного шару/UC → за §12.6 MUST decompose у LOW/VERY-LOW листя перед кодом.

## Sub-tasks (DECOMPOSED)
| Sub-task ID | Title | Estimation | Status | Depends On |
|---|---|---|---|---|
| TASK-117-A | Tracking Plan v1 контракт (~20 подій + property-схема + validator const) — розширення `telemetry.rs` name/props allow-list, нуль нових таблиць | LOW | READY | TASK-101 |
| TASK-117-B | Серверна емісія у point-of-emit: `signup`, `checkout_start`, `paid`, `rsvp`, `ticket_claimed`, `scan_result` — інлайн `telemetry::log` у наявних хендлерах (`orders.rs`,`tickets.rs`,`payments.rs`,`webhooks.rs`,`eventsocial.rs`,`auth.rs`) | LOW | READY | TASK-117-A |
| TASK-117-C | Anti-abuse на публічному `POST /api/telemetry`: per-IP/per-`anon_id` rate-limit + props size/allow-list guard (SEC-1..4) | VERY-LOW | READY | TASK-117-A |
| TASK-117-D | Funnel-агрегація: `GET /api/admin/telemetry/funnel` (view→…→attend, drop%, timeseries, сегментація source/channel) — read-only SQL over `telemetry_events` | LOW | READY | TASK-117-B |
| TASK-117-E | K-factor: `GET /api/admin/telemetry/kfactor` — парна `*_sent↔*_accepted` + `attributed_signup` через `share_token`/`token_nonce`; anon_id→user_id merge міст | LOW | READY | TASK-117-B |
| TASK-117-F | Android емісія (`android/.../ui/**`): `event_view`,`detail_open`,`tier_select`,`checkout_start`,`share_clicked`,`invite_sent`,`onboarding_step` через `POST /api/telemetry` (best-effort, не ламає дію) | LOW | READY | TASK-117-A |
| TASK-117-G | Web емісія (`web/app/**` public client): `event_view`,`detail_open`,`share_link_visited`,`guest_signup`,`re_share`; `anon_id` cookie/localStorage seam | LOW | READY | TASK-117-A, TASK-016 |
| TASK-117-H | Admin north-star дашборд (`admin/app/telemetry`): NSM-картка, funnel з drop%, K-factor, channel-breakdown, timeseries | LOW | READY | TASK-117-D, TASK-117-E |
| TASK-117-I | (INFRA, за потреби) GIN-індекс на `props` + `(name, created_at)` покриття для funnel-агрегатів — міграція 0033 | VERY-LOW | READY | TASK-117-D |

## Execution Order
1. **TASK-101** (spec: north-star/K-factor/пороги + нові UC) — governing prerequisite.
2. **TASK-117-A** (event-contract) — фіксує канонічний перелік і props перед будь-якою емісією.
3. **TASK-117-B** + **TASK-117-C** (серверна емісія + anti-abuse публічного endpoint) — паралельно.
4. **TASK-117-F**, **TASK-117-G** (клієнтська емісія Android/Web) — паралельно, після A.
5. **TASK-117-D**, **TASK-117-E** (funnel + K-factor API) — після наявності подій (B; клієнтські бажані).
6. **TASK-117-I** (індекс) — лише якщо профілювання D показує повільні агрегати.
7. **TASK-117-H** (admin дашборд) — останнім, споживає D+E.

## Speck Reference
docs/specks/SDD-117-analytics-tracking.md
