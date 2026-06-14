# SDD-008 — Платежі: test-режим + admin-конфіг + ескроу (backend)

**Backlog:** TASK-008  **Impacted UCs:** UC-004, UC-010, UC-011

## Scope / Non-goals
**Scope:** payment_settings (admin-конфіг), admin-роль/extractor/акаунт, orders+escrow, checkout, pay-test (симуляція).
**Non-goals:** Android UI, web-адмінка, реальний LS live + webhook, кліринг/виплати ескроу.

## Design
**Міграція 0005:** `payment_settings` (singleton: `mode`, `provider`, `ls_api_key`, `ls_store_id`, `ls_variant_id`,
`platform_fee_bps` дефолт 1000; seed рядок test); `orders` (event_id, buyer_id, amount, currency, status pending/paid/failed,
escrow_status none/held/cleared/refunded/paid_out, provider, ticket_id).
**Auth:** `CurrentAdmin` extractor = `CurrentUser` + перевірка `role='ADMIN'` (інакше 403 Forbidden).
**Payments API:** `GET /api/admin/payment-settings` (ключі → `ls_api_key_set: bool`, не значення); `PUT` (часткове оновлення, валідація `mode∈{test,live}`, `provider∈{test,lemonsqueezy}`).
**Orders:** `POST /api/events/:id/checkout` (authed, paid+published+availible) → order pending → `{order_id, mode, amount, test_mode}`;
free → 409. `POST /api/orders/:id/pay-test` (authed, власник, order pending, **mode=test**) → транзакція: декремент `tickets_left` → мінт квитка (price=amount) → order `status=paid, escrow=held, ticket_id` → повертає квиток.
**Ticket mint refactor:** `tickets::decrement_tickets` + `tickets::mint_ticket` (спільні для reserve і pay-test).

## Acceptance Criteria
- **AC-1** Дефолт settings = `mode=test, provider=test`; GET (admin) маскує ключі (`ls_api_key_set`).
- **AC-2** PUT (admin) частково оновлює; невалідні `mode/provider` → 422.
- **AC-3** Не-admin на `/api/admin/*` → 403; без токена → 401.
- **AC-4** checkout платної події (authed) → 201 order pending, `test_mode=true`; free → 409; sold-out → 409.
- **AC-5** pay-test (власник, test-режим) → 201 квиток; order `paid`+`held`; квиток у `/api/me/tickets`; `tickets_left` зменшено.
- **AC-6** pay-test чужого/обробленого order → 404/409; pay-test у live-режимі → 409.

## Security Acceptance Criteria
- **SEC-1** Admin-ендпоінти лише для `role=ADMIN` (403), решта — 401 без токена.
- **SEC-2** Секрети LS не повертаються в GET (лише `*_set: bool`).
- **SEC-3** pay-test лише власником order і лише в test-режимі (анти-зловживання).
- **SEC-4** Транзакція проти over-sell (атомарний декремент); buyer_id/owner з токена.

## Test Matrix
| AC | Integration |
|---|---|
| AC-1 | ✅ default + masked GET |
| AC-2 | ✅ PUT partial + 422 |
| AC-3/SEC-1 | ✅ non-admin 403, no-token 401 |
| AC-4 | ✅ checkout paid 201 / free 409 |
| AC-5 | ✅ pay-test → ticket + order paid/held + in my tickets |
| AC-6/SEC-3 | ✅ live-mode pay-test 409; foreign order 404 |
