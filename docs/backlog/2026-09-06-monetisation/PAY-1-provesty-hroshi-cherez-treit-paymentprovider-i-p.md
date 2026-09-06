# PAY-1 — Провести гроші через трейт PaymentProvider і почати зберігати те, що сказав провайдер

| Поле | Значення |
|---|---|
| **Фаза** | Фаза 1 — гроші |
| **Estimation** | L |
| **Status** | `INTAKE` |
| **Критичний шлях до першого платежу** | так |
| **Depends on** | — |

## Навіщо

Сьогодні seam мертвий: `provider::resolve` і трейт `PaymentProvider` не викликаються ніде, окрім власних юніт-тестів (перевірено грепом по api/src). `orders::checkout` будує LS-URL інлайном через `format!("https://{store}.lemonsqueezy.com/buy/{variant}?...")`, а `webhooks.rs` сам рахує HMAC і одразу кличе `orders::fulfill`. Тобто 100% покриття покриває мертвий код, а вставити реальний PSP нікуди. Додатково: `orders` не зберігає жодного ідентифікатора платежу — тому зробити рефанд через провайдера фізично нічим, у `RefundReq.provider_ref` нема що покласти. Це блокер, а не покращення. Задача розширює наявні `provider.rs`/`orders.rs`/`webhooks.rs`, нічого не створює заново, і виконується ПОВНІСТЮ на TestProvider — не залежить ні від юрособи, ні від відповіді Stripe.

## Як це працює

Для людини не змінюється нічого: покупець так само тисне «Купити», отримує квиток у test-режимі, адмін так само бачить замовлення. Змінюється те, що система пише про кожен рух грошей. Покупець тисне «Купити» → `checkout` більше не склеює URL руками, а питає активного провайдера: «збери N у валюті C за замовлення X» → провайдер повертає `ChargeResult{provider_ref, checkout_url, status}` → ми зберігаємо `provider_ref` у замовленні ще ДО того, як покупець піде платити. Провайдер підтверджує оплату вебхуком → ми спершу записуємо `(provider, event_id)` у журнал подій, і лише якщо такої пари ще не було — фулфілимо. Повторна доставка того самого вебхука не робить нічого і повертає 200. Якщо в налаштуваннях стоїть ім'я провайдера, якого код не знає, сервіс НЕ підміняє його мовчки тестовим — checkout повертає 409 «платіжний провайдер не налаштований», і адмін бачить це на екрані платіжок.

## Зміни в API

Без нових публічних ендпоінтів. `orders::checkout` більше не формує URL самостійно: викликає `provider::resolve(&settings.provider)?.create_charge(ChargeReq{..})` і повертає `checkout_url` з `ChargeResult`. `webhooks.rs` викликає `provider.verify_webhook(&headers, &body) -> WebhookEvent` і маршрутизує: `Paid{order_ref}` → `orders::fulfill`, `Refunded{order_ref}` → шлях рефанду, `Ignored` → 200. `provider::resolve` міняє сигнатуру на `resolve(name: &str) -> Result<Box<dyn PaymentProvider>, ProviderError>` і повертає помилку на невідомому імені (наявний тест `resolve_maps_names_and_defaults_to_test`, що очікує `resolve("lemonsqueezy").name() == "test"`, переписується — це не послаблення тесту, а зміна контракту, зафіксувати в коміті). `ChargeReq` += `connected_account: Option<String>`, `application_fee: Option<Decimal>`; `RefundReq` += `connected_account: Option<String>`, `refund_application_fee: bool`; `PayoutReq` += `connected_account: Option<String>`. `WebhookEvent` += `Disputed{order_ref}`, `PayoutPaid{payout_ref}`, `PayoutFailed{payout_ref, reason}`, `AccountUpdated{account_ref, charges_enabled, payouts_enabled, requirements_due}`. Докстрінг `transfer_payout` фіксує нову семантику: це виплата з балансу продавця на його банк, а не переказ платформа→організатор.

## Зміни в базі

Міграція 0040: `ALTER TABLE orders ADD COLUMN provider_ref TEXT`, `ADD COLUMN connected_account TEXT`, `ADD COLUMN application_fee NUMERIC(12,2)`, `ADD COLUMN refunded_amount NUMERIC(12,2) NOT NULL DEFAULT 0`; `CREATE UNIQUE INDEX orders_provider_ref_uidx ON orders (provider, provider_ref) WHERE provider_ref IS NOT NULL`; `CREATE INDEX orders_escrow_idx ON orders (escrow_status)`. Нова таблиця `webhook_events (provider TEXT NOT NULL, event_id TEXT NOT NULL, kind TEXT NOT NULL, order_ref TEXT, received_at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY (provider, event_id))`. `ALTER TABLE orders ADD COLUMN terminal_event_seen TEXT` — прапорець «уже прийшла термінальна подія» (`refunded`/`disputed`), щоб позачергова доставка не дала виписати квиток на вже повернений платіж.

## Поверхні

**Веб.** Немає видимих змін для покупця. На адмін-екрані платіжок з'являється рядок «провайдер розпізнаний: так/ні» (з readiness).

**Android.** Немає видимих змін; клієнт як і раніше відкриває `checkout_url`, якщо він прийшов.

**Telegram Mini App.** Немає (TMA будується у Фазі 5 і підключиться до того самого `checkout_url`).

## Критерії приймання (AC)

- Греп по `api/src` показує щонайменше один виклик `provider::resolve` поза `provider.rs` (у `orders.rs` і в `webhooks.rs`).
- `checkout` на TestProvider створює замовлення з непорожнім `orders.provider_ref`; повторний `checkout` тим самим покупцем створює НОВЕ замовлення з іншим `provider_ref` (унікальний індекс не порушується).
- `resolve("невідоме_ім'я")` повертає `Err`, а `checkout` при такому `payment_settings.provider` повертає 409 і НЕ створює рядок в `orders`.
- Той самий вебхук, доставлений двічі (однаковий `event_id`), фулфілить рівно один раз: у `tickets` один рядок, у `webhook_events` один рядок, обидві відповіді 200.
- Вебхук `Refunded`, доставлений ПЕРЕД `Paid` для того самого замовлення, ставить `terminal_event_seen='refunded'`; наступний `Paid` не мінтить квиток і повертає 200.
- Юніт-тести на всі нові варіанти `WebhookEvent` (кожен варіант хоч раз побудований і замачений), покриття лишається 100% за `scripts/ci.sh`.

## Що має бути відхилено (SEC)

- Вебхук без підпису або з підписом під іншим секретом — 401, `webhook_events` не поповнюється, `fulfill` не викликається.
- Невідоме ім'я провайдера НЕ мапиться на TestProvider: у live-режимі це мовчазне перетворення реального платежу на безкоштовний квиток — має бути 409.
- `provider_ref` з чужого замовлення (підміна `order_ref` у тілі вебхука) не дає зафулфілити чуже замовлення: збіг перевіряється по парі (provider, provider_ref), і невідповідність — 200 без дії, не 500.
- Дві паралельні доставки того самого `event_id` не проходять обидві: конфлікт на PRIMARY KEY `webhook_events` призводить до no-op, а не до другого квитка.
- `ProviderError` ніколи не потрапляє в тіло HTTP-відповіді дослівно (може містити ключі/сирі payload-и) — назовні йде нейтральний текст.
