# TASK-008 — Платежі: test-режим + admin-конфіг + ескроу (фундамент)

**Status:** IN PROGRESS
**Created:** 2026-06-12  **Updated:** 2026-06-12
**Parent:** TASK-001 (UC-004/UC-010/UC-011)  **Speck:** docs/specks/SDD-008-payments-testmode.md

## Raw Request
> «Для платіжки підготуй можливість через адмінку додати все, а поки тестовий режим, щоб тестувати флоу.»

## Refined Description
**Scope (цей крок — backend):** `payment_settings` (single-row, БД, admin-конфіг; дефолт `mode=test`, `provider=test`,
LS-поля для майбутнього); admin-роль + `CurrentAdmin` extractor + сід `admin@underground.dev`; `GET/PUT /api/admin/payment-settings`
(ключі маскуються в GET); `orders` + ескроу-стани; `POST /api/events/:id/checkout` (платна подія → order pending);
`POST /api/orders/:id/pay-test` (лише test-режим → симулює успіх: мінт квитка + `escrow=held` + order paid).
**Non-goals (наступні кроки):** Android платний-тест-UI; сторінка адмінки (web); реальний Lemon Squeezy checkout+webhook (live);
кліринг/виплати ескроу (`held→cleared→paid_out`).
**Impacted UCs:** UC-004, UC-010, UC-011. **Dependencies:** TASK-006 (квитки).

## Estimation
**Level:** HIGH → ріжемо: цей таск = backend-фундамент (test-флоу повністю тестується curl); UI/web/live — окремі кроки.

## Speck Reference
docs/specks/SDD-008-payments-testmode.md

## Verification — BACKEND (2026-06-12)
Міграція 0005 (payment_settings singleton seed test + orders+escrow); `CurrentAdmin` extractor (403); admin `GET/PUT /api/admin/payment-settings`
(ключі маскуються → `ls_api_key_set`); `POST /api/events/:id/checkout` (paid→order pending) + `POST /api/orders/:id/pay-test` (test-only→мінт квитка+escrow held).
Ticket mint винесено в `tickets::decrement_tickets`/`mint_ticket` (спільні з reserve). **35 тестів** зелені (payments_flow: default+masked, PUT partial+422, non-admin 403, checkout paid 201/free 409, pay-test→ticket, foreign order 404, double-pay 409). clippy/fmt чисто.
Curl-смоук: admin PUT (fee 1200, store_42, key masked) ✓; non-admin 403 ✓; user checkout LOUNA (test_mode, 650) → pay-test → квиток мінт, order paid/held ✓.
Сід: `admin@underground.dev` (ADMIN, пароль `underground`).

## Verification — ANDROID paid test-flow (2026-06-12)
EventApi `checkout`/`payTest` + `CheckoutResp`; `TicketsViewModel.checkout/payTest`; `TestPayScreen` (бейдж ТЕСТОВИЙ РЕЖИМ, сума, «Сплатити (тест)»);
детальний екран: платна подія → `onBuy` → checkout → `pay/{order}/{amount}/{currency}` → pay-test → e-квиток. Навігація без тупіків (топ-бар back).
**Перевірено на moto g05:** Настолки (120 UAH) → Купити → екран оплати (120.00 UAH, тест) → Сплатити (тест) → e-квиток QR (`UG-11b2ef70…`).
API: квиток 120.00 у користувача, order `paid`+escrow `held`. Android-білд чистий.

**Статус: test-режим платежів — DONE** (backend + Android, повний флоу тестується).

## Залишок (окремі кроки)
- **Web-адмінка (TASK-001-F):** сторінка налаштувань платежів (ввід LS-ключів/mode) поверх готового API + модерація.
- **Live LS:** реальний hosted checkout + webhook (mode=live); кліринг ескроу held→cleared→paid_out; виплати організаторам.
