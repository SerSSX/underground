# SDD-104 — Payment-provider seam: trait + Noop/Test провайдери + ledger

**Impacted UCs:** UC-004, UC-005, UC-010, UC-011 (нова provider-neutral поведінка та `ledger_entries` — spec/BR-текст оновлюється у TASK-101; BR-004/BR-005 → «активний PaymentProvider»)
**Impacted BR/WF:** BR-003, BR-004, BR-005, BR-006; WF-Escrow (незмінний — seam викликається з-під `escrow.rs`)

## Scope / Non-goals
**Scope:**
- `api/src/payments/provider/mod.rs`: `trait PaymentProvider { create_charge, capture, hold, transfer_payout, refund, verify_webhook }` + provider-neutral DTO (`ChargeReq/ChargeResult`, `PayoutReq/TransferResult`, `RefundReq/RefundResult`, `WebhookEvent`) з `amount_minor:i64`+`currency` (контракт TASK-CONTRACTS-1) і без жодного LS-специфічного поля.
- `NoopProvider` (free/RSVP), `TestProvider` (admin mark-paid/mark-settled, детерміновані `provider_ref`).
- Registry `from_settings(provider, provider_config)` → `noop|test|lemonsqueezy` за `payment_settings.provider`.
- `ledger_entries` (append-only, double-entry) + writer при mint/hold/sweep/payout/refund.
- Інтеграція `orders.rs`/`payouts.rs`/`webhooks.rs` за флагом `PAYMENT_SEAM_ENABLED` (OFF-by-default).

**Non-goals:** `LemonSqueezyProvider` за trait, generic `/api/webhooks/:provider`, видалення LS-гілок, live-mode creds, реальні перекази, ticket tiers, клієнтський `provider_action` UI, зміна формул escrow/комісії — окремі задачі (Фаза 4 / TASK-CHECKOUT-1).

## Acceptance Criteria
- AC-1: `trait PaymentProvider` визначено з рівно шістьма методами (`create_charge/capture/hold/transfer_payout/refund/verify_webhook`), усі суми — `amount_minor:i64`+`currency`; жоден DTO не містить провайдер-специфічних полів.
- AC-2: `NoopProvider::create_charge` повертає `ChargeResult{status=Paid, checkout_url=None, provider_ref=<deterministic>}` без будь-яких зовнішніх/мережевих викликів; застосована комісія = 0.
- AC-3: `TestProvider::create_charge` повертає `status=Pending` з `provider_action=test_autopay`; admin `mark-paid`/`mark-settled` детерміновано просувають order→fulfill та payout→settled; `provider_ref` детермінований і стабільний між запусками.
- AC-4: Registry `from_settings("noop"|"test"|"lemonsqueezy", cfg)` повертає відповідну реалізацію; невідоме значення → fail-fast `AppError` конфігурації (не паніка, не тихий дефолт).
- AC-5: Міграція `ledger_entries` створює таблицю з полями `entry_type, order_id|payout_id, amount_minor BIGINT, currency, direction ('debit'|'credit'), provider_ref, created_at`; UPDATE/DELETE заборонені на рівні БД (REVOKE або тригер).
- AC-6: Ledger-writer вставляє парні записи при mint(credit HELD)/payout(debit)/refund(reverse); сума debit == сума credit у межах транзакції для завершеного циклу order.
- AC-7: escrow.rs лишається provider-neutral — жодного розгалуження на `provider == "..."` в escrow/tickets/orders домені; provider викликається лише через trait.
- AC-8: З `PAYMENT_SEAM_ENABLED=false` (default) наявний Lemon Squeezy шлях (`orders.rs` checkout URL, `webhooks.rs` HMAC) незмінний; існуючі LS-інтеграційні тести проходять без правок.
- AC-9: `./scripts/ci.sh` зелений і з флагом OFF, і з ON (Noop/Test активні).

## Security Acceptance Criteria
- SEC-1 [no-real-money]: Noop/Test провайдери НЕ виконують жодних зовнішніх переказів; спроба `transfer_payout` під Noop/Test лишається симуляцією і фіксується в ledger, реальні гроші не рухаються.
- SEC-2 [ledger-immutability]: будь-який `UPDATE`/`DELETE` на `ledger_entries` відхиляється БД (negative-тест ловить помилку доступу); історія append-only.
- SEC-3 [authz-IDOR]: admin `mark-paid`/`mark-settled` (TestProvider) вимагають `CurrentAdmin`; звичайний user / чужий order → 403/404, не просування стану.
- SEC-4 [webhook-replay]: `verify_webhook` відхиляє повтор події (той самий `provider_ref`+kind не просуває стан двічі — ідемпотентність за `provider_ref`).
- SEC-5 [config-injection]: `provider` з `payment_settings`/`provider_config` не інтерполюється у SQL/шелл; невідомий провайдер → fail-fast, не виконання довільного шляху.
- SEC-6 [secret-leak]: `provider_config`/webhook-секрети не потрапляють у логи, error-response чи ledger; лише маскований `provider_ref` зберігається.
- SEC-7 [amount-overflow]: `amount_minor` за межами `i64`/від'ємне для charge → валідаційна помилка, не мовчазне переповнення чи негативний ledger-баланс.

## Failure Modes / Error Mapping
| Умова | Обробка |
|---|---|
| Невідомий `provider` у settings | Fail-fast `AppError::Config` на старті/резолві registry (500 з generic msg, без leak) |
| Noop/Test отримує paid-charge на past/age-gated подію | Гейти домену спрацьовують ДО seam → 409, provider не викликається |
| Повторний webhook (той самий `provider_ref`) | Ідемпотентно ігнорується (SEC-4), стан не просувається двічі |
| UPDATE/DELETE на `ledger_entries` | Помилка доступу БД (SEC-2); транзакція відкочується |
| `amount_minor` ≤ 0 або overflow | `AppError::Validation` (SEC-7) |
| Відсутній `PAYMENT_SEAM_ENABLED` | Default `false` → LS-шлях, seam неактивний (AC-8) |
| admin mark-paid без прав | 403 (SEC-3) |

## Test Matrix
| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-2 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-3 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-4 | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-5 | ⬜ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-6 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-7 | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-8 | ⬜ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-9 | ⬜ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-1 | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-2 | ⬜ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| SEC-3 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-4 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-5 | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-6 | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-7 | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |

_UI/Base UI стадії порожні: задача суто API/Infra (seam + міграція), user-facing поверхонь немає — клієнтський `provider_action` та checkout UI покриваються TASK-CHECKOUT-1._
