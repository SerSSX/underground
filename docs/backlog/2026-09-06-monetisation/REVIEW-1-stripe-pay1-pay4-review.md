# REVIEW-1 — Незалежне рев'ю PAY-1 + PAY-4 (коміт 233ce8a) і його виправлення

| Поле | Значення |
|---|---|
| **Тип** | Review record + fix task |
| **Status** | `IN PROGRESS` — виправлення закомічені, повторне рев'ю іншою ідентичністю ще не проведене |
| **Дата рев'ю** | 2026-09-08 |
| **Рев'юер** | окремий агент (opus, xhigh), інструктований на absence review; автор коду участі не брав |
| **Вердикт рев'ю** | **мерджити не можна** — 4 CRITICAL, 9 MAJOR, 5 MINOR; мутація «прибрати перевірку підпису» лишала всі 90 юніт-тестів зеленими |

## Знахідки та рішення

Кожен рядок — прийнято / відхилено / перетворено на тест. Порожніх рішень немає.

| # | Sev | Файл | Суть | Рішення |
|---|---|---|---|---|
| 1 | CRITICAL | stripe.rs | `metadata[order_id]` лише на Checkout Session; на Charge/Dispute не потрапляє → refund/dispute ніколи не знаходять замовлення, escrow доводить до виплати вже повернені гроші | **Прийнято.** Додано `payment_intent_data[metadata][order_id]`; `apply` резолвить замовлення і по `provider_ref` (PaymentIntent), не лише по UUID з метаданих. Тест: `a_dispute_without_metadata_is_traced_through_the_payment_intent_and_freezes`, assert на поле форми в `stripe_http.rs` |
| 2 | CRITICAL | webhooks.rs | Дедуп записується ДО побічного ефекту окремим автокомітом; провал `fulfill` → 4xx, а event_id уже спалений | **Прийнято.** `record_delivery` у тій самій транзакції, що й ефект; `fulfill` не падає за бізнес-причиною (PAY-3) |
| 3 | CRITICAL | webhooks.rs | `Paid` фулфілить будь-яке замовлення з `metadata.order_id`; нема прив'язки до збереженого `provider_ref`, до провайдера, до суми. Експлуатується через LS `custom_data` сьогодні, через Stripe metadata після PAY-5 | **Прийнято.** Прив'язка: `orders.provider == provider`, `orders.provider_ref == charge_ref` (id сесії), `to_minor(amount) == amount_total`, валюта збігається. Тести: `a_paid_event_names_another_order_in_metadata_and_is_ignored`, `..._different_amount_or_currency...`, `..._cannot_touch_an_order_from_another_provider`, LS-тест переписано на «не може фулфілити» |
| 4 | CRITICAL | payments.rs, provider.rs | Live-ключ приймається в test-режимі; `resolve` не читає `mode`; `moves_real_money()` не викликається ніде | **Прийнято.** Дзеркальна перевірка в `update_settings`; `resolve` відмовляє при розходженні `kind ↔ mode` і при `mode=live` без провайдера, що рухає гроші. Юніт-тести `resolve_refuses_*` |
| 5 | MAJOR | webhooks.rs | Рефанд лишає квиток активним, місце зайнятим; часткові рефанди переводять усе замовлення в `refunded` | **Прийнято.** `orders::apply_full_refund` (квиток `returned`, місце назад, промо назад, escrow `refunded`) — лише коли `amount_refunded >= amount`; частковий → `refunded_amount`. Тест `a_partial_refund_is_recorded_and_a_full_one_returns_the_ticket` |
| 6 | MAJOR | stripe.rs | Прибрати виклик перевірки підпису — усі тести зелені | **Прийнято.** Інтеграційний тест через маршрут: `a_signature_under_another_secret_is_refused_and_counted` |
| 7 | MAJOR | Cargo.toml | `wiremock` оголошений і не використаний; HTTP-половина адаптера без покриття | **Прийнято.** `tests/stripe_http.rs`: заголовки, ключі ідемпотентності, 429×2→200, 503×3→помилка, 4xx без ретраю, без ключа — без ретраю, не-JSON, нема id, надто велике тіло, `check_connection` ok/401, невалідні суми не доходять до мережі |
| 8 | MAJOR | orders.rs | Текст `ProviderError` віддається покупцю дослівно | **Прийнято.** У лог; покупцю фіксований рядок. Виняток — `Invalid` з наших власних пре-перевірок (мінімум валюти, комісія ≥ сума): вони actionable і про нас нічого не кажуть |
| 9 | MAJOR | webhooks.rs | Дедуп LS за `data.id` (id замовлення) — перша подія блокує всі наступні | **Прийнято.** `meta.webhook_id` |
| 10 | MAJOR | tests/webhook.rs | Червоний тест `checkout_live_returns_lemonsqueezy_url` | **Прийнято.** Переписано під новий контракт: live+lemonsqueezy → 409 до створення замовлення й без холду місця |
| 11 | MAJOR | orders.rs | Видалений охоронець live: live+lemonsqueezy створює неоплачувані замовлення | **Прийнято.** `lemonsqueezy` у `resolve` → Err (retired); `mode=live` вимагає `moves_real_money()`. Старий тест `checkout_blocked_in_live_without_variant` зеленіє без змін |
| 12 | MAJOR | StripeSection.tsx | Збережений ключ неможливо очистити з UI | **Прийнято.** Кнопки «Видалити секретний ключ» / «Видалити секрет вебхука» з підтвердженням; шлють `""` |
| 13 | MAJOR | payments.rs | Неправильний секрет вебхука не відрізнити від правильного | **Прийнято.** Міграція 0042: `stripe_last_webhook_at`, `stripe_webhook_rejections` (скидаються при зміні секрету); рядок «Вебхук підтверджений» у чеклісті |
| 14 | MINOR | stripe.rs | `reqwest::Client` на кожен запит; вебхук без rate-limit | **Прийнято.** `OnceLock` клієнт; `ratelimit::hit` 600/хв на обидва маршрути (глобальний бакет — IP у хендлер не проведено; IP-бакет — окремою задачею) |
| 15 | MINOR | stripe.rs | Округлення мінорних одиниць розходиться з `orders.amount` | **Частково.** `from_minor` для зворотної конвертації; збереження списаної суми в мінорних одиницях — це PAY-2, не тут |
| 16 | MINOR | admin/lib/api.ts | Тіло помилки 422 викидається, оператор бачить лише статус | **Прийнято.** `updateSettings` через `apiSend` (читає `error`). Статус 422 залишено (Validation), а не 409 як в AC — змінювати коди під наявні тести без потреби не стали; зафіксовано тут |
| 17 | MINOR | scripts/ci.sh | Адмінка не типізується й не лінтиться в гейті | **Прийнято частково.** Стадія `admin typecheck` (`tsc --noEmit`, зелений). `next lint` НЕ додано: в `admin/` немає конфігу ESLint, і без нього `next lint` відкриває інтерактивний промпт — CI зависав би. Конфіг ESLint — окрема задача, після неї стадія lint |
| 18 | MINOR | stripe.rs | `STRIPE_API_BASE` приймає `http://` на будь-який хост | **Прийнято.** Лише `https://` або loopback `http://`; інакше `Err`. Юніт-тест |

## Що НЕ зроблено і відомо

- IP-keyed rate limit на вебхуки (потрібен `ConnectInfo` у роутері) — окремою задачею.
- Таймаут-тест клієнта (20 с) — не написаний, бо коштував би 20 с на прогін; таймаут задекларований у `http_client()`.
- Повторне незалежне рев'ю цього виправлення ще не проведене — до нього статус `IN PROGRESS`.
