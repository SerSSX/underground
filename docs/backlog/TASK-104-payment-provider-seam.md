# TASK-104 — Payment-provider seam: trait + Noop/Test провайдери + ledger

**Status:** IN PROGRESS (leaf 104-A DONE)
**Created:** 2026-06-29  **Updated:** 2026-07-02

> **Реалізовано (verified `cargo test`, 7 unit-тестів зелені):** leaf **104-A — provider seam** у
> `api/src/provider.rs` + `lib.rs`: `trait PaymentProvider` (create_charge/transfer_payout/refund/verify_webhook +
> name/moves_real_money), provider-neutral DTO на `Decimal` (узгоджено з BR-019, НЕ i64), `NoopProvider` (free/RSVP,
> лише zero-charge), `TestProvider` (paid у тест-режимі без реальних грошей), `resolve()` (product-first: невідомий/
> реальний PSP → Test). Не чіпає наявний Lemon Squeezy шлях. Повний API-суіт: 217 passed / 0 failed.
> **Залишок (LOW-листя):** `ledger_entries` міграція + запис; wiring у `orders.rs`/`webhooks.rs` за
> `payment_settings.provider` (за флагом, без поломки LS); справжній LS/Stripe-адаптер за trait — Фаза 3–4.

## Raw Request
> Payment-provider seam: `trait PaymentProvider` (create_charge/hold/capture/transfer_payout/refund/verify_webhook) + `NoopProvider` (free/RSVP) + `TestProvider` (admin mark-paid) + registry за `payment_settings.provider`; `ledger_entries` (append-only). НЕ ламати наявний Lemon Squeezy шлях (за флагом). `escrow.rs` лишається provider-neutral. Ініціатива INIT-PROV, Phase 0, оцінка HIGH.

## Refined Description
**Scope:**
- Новий модуль `api/src/payments/provider/mod.rs` з `trait PaymentProvider` (`create_charge`, `capture`, `hold`, `transfer_payout`, `refund`, `verify_webhook`) та provider-neutral DTO (`ChargeReq/ChargeResult`, `PayoutReq/TransferResult`, `RefundReq/RefundResult`, `WebhookEvent`, enum-и `ChargeStatus`/`PayoutStatus`/`WebhookKind`).
- `NoopProvider` — безкоштовний/RSVP шлях: `create_charge` миттєво повертає `status=Paid`, `checkout_url=None`; жодних зовнішніх викликів; комісія = 0.
- `TestProvider` — детермінований платний цикл для CI/демо: `create_charge` повертає `provider_action=test_autopay`, admin `mark-paid`/`mark-settled` просувають order/payout; детерміновані `provider_ref`.
- Registry `PaymentProvider::from_settings(payment_settings.provider, provider_config)` → `noop|test|lemonsqueezy`; невідомий провайдер → fail-fast помилка конфігурації.
- Міграція `ledger_entries` (append-only, double-entry: `direction debit|credit`, `amount_minor:i64`, `currency`, `entry_type`, `order_id|payout_id`, `provider_ref`, `created_at`); запис при mint/hold/sweep/payout/refund.
- Інтеграція `orders.rs`/`payouts.rs`/`webhooks.rs` викликає seam замість inline-логіки — **за feature-флагом** `PAYMENT_SEAM_ENABLED`; при OFF наявний Lemon Squeezy шлях лишається байт-у-байт незмінним.

**Non-goals:**
- `LemonSqueezyProvider` за trait, generic `POST /api/webhooks/:provider`, видалення LS-гілок з домену — Фаза 4 (TASK-PROV-LS-1).
- Реальні перекази грошей / live-mode PSP creds.
- Зміна формул escrow/комісії, ticket tiers, checkout UI/`provider_action` на клієнтах (TASK-TIERS-1/TASK-CHECKOUT-1).
- Реальний ledger-transport (реальні гроші) — увімкнення у Фазі 4.

**Impacted UCs:** UC-004 (Купівля), UC-005 (Повернення), UC-010 (Ескроу-розрахунок), UC-011 (Адмін: фінанси). Нова provider-neutral поведінка seam та `ledger_entries` — **new UC/BR text required (див. TASK-101 spec reconciliation)**: BR-004/BR-005 переформулювати з «Lemon Squeezy» на «активний PaymentProvider».

**Impacted BR/WF:** BR-003 (комісія), BR-004 (карткові дані не зберігаються — тепер provider-neutral), BR-005 (refund через активний провайдер), BR-006 (гроші лише через seam `escrow.rs` — seam лишається єдиним дотиком, provider викликається з-під нього). WF-Escrow незмінний.

**Dependencies:**
- TASK-101 (spec reconciliation: фіксує порядок фаз + provider-neutral BR-004/BR-005) — **hard, ПЕРЕД кодом**.
- TASK-CONTRACTS-1 (`money.rs`, канон `amount_minor:i64` + `currency`) — **hard**: `ledger_entries.amount_minor` та DTO споживають цей контракт.
- Наявні модулі (розширюються, не переписуються): `api/src/payments.rs` (`load_settings`), `api/src/orders.rs`, `api/src/payouts.rs`, `api/src/webhooks.rs`, `api/src/escrow.rs`.

**Risks / Open Questions:**
- Ризик поламати LS CI при інтеграції → мітигація: інтеграція лише за флагом OFF-by-default; LS-інтеграційні тести незмінні.
- `verify_webhook` без реального PSP — Noop/Test повертають детерміновані події; HMAC-верифікація повноцінно тестується лише у Фазі 4 (тут — контракт + Test-детермінізм).
- Append-only enforcement: тригер/`REVOKE UPDATE,DELETE` на рівні БД — узгодити з міграційним стилем (лише DDL, §10).

## Estimation
**Level:** HIGH  **Justification:** Зачіпає ≥4 шари домену (новий trait-модуль, 2 реалізації, registry+config, міграція БД, інтеграція трьох існуючих модулів за флагом), перетинає кілька UC/BR і несе ризик регресії платіжного шляху. За §12.5/§12.6 HIGH **MUST be decomposed** у LOW-листя перед кодом.

## Sub-tasks (DECOMPOSED)
| Sub-task ID | Title | Estimation | Status | Depends On |
|---|---|---|---|---|
| TASK-104-A | Infra: міграція `0033_ledger.sql` — `ledger_entries` append-only (DDL + REVOKE UPDATE/DELETE) | LOW | READY | TASK-CONTRACTS-1 |
| TASK-104-B | API: `trait PaymentProvider` + provider-neutral DTO/enum-и в `api/src/payments/provider/mod.rs` (без impl) | LOW | READY | TASK-101, TASK-CONTRACTS-1 |
| TASK-104-C | API: `NoopProvider` impl (free/RSVP: instant Paid, комісія=0) + unit+[SEC] | LOW | READY | 104-B |
| TASK-104-D | API: `TestProvider` impl (детерміновані `provider_ref`, admin mark-paid/mark-settled) + unit+[SEC] | LOW | READY | 104-B |
| TASK-104-E | API: registry `from_settings` + config `provider_config JSONB` (міграція `0034_payment_provider.sql`) + fail-fast невідомого провайдера | LOW | READY | 104-C, 104-D |
| TASK-104-F | API: ledger-writer (append-only insert при mint/hold/sweep/payout/refund) поверх escrow seam | LOW | READY | 104-A, 104-E |
| TASK-104-G | API: інтеграція `orders.rs`/`payouts.rs`/`webhooks.rs` за флагом `PAYMENT_SEAM_ENABLED` (OFF-by-default; LS шлях незмінний) | LOW | READY | 104-E, 104-F |

## Execution Order
1. TASK-104-A (schema ledger) паралельно з TASK-104-B (trait/DTO) — незалежні.
2. TASK-104-C, TASK-104-D (реалізації Noop/Test) — обидві після 104-B.
3. TASK-104-E (registry+config) — після C+D.
4. TASK-104-F (ledger-writer) — після A+E.
5. TASK-104-G (інтеграція за флагом) — остання, після E+F; `./scripts/ci.sh` зелений з флагом і OFF, і ON.

## Speck Reference
docs/specks/SDD-104-payment-provider-seam.md
