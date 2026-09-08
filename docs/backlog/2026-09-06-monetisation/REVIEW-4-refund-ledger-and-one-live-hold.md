# REVIEW-4 — Четверте незалежне рев'ю (мутаційне підтвердження REVIEW-3 + absence review) і його виправлення

| Поле | Значення |
|---|---|
| **Тип** | Review record + fix task |
| **Status** | `IN PROGRESS` — виправлення в коді; п'яте рев'ю (підтвердження) ще не проведене |
| **Дата рев'ю** | 2026-09-09 |
| **Рев'юер** | окремий агент (opus); 12 мутацій по REVIEW-3 (усі вбиті), власні проби на двох з'єднаннях, 164 тести зелені на `dae241d` |
| **Вердикт рев'ю** | **мерджити не можна**: 2 MAJOR, 4 MINOR, 1 TRIVIAL; spec BR-023/024 знову колізують |
| **Ревʼюване** | `dae241d` (база `9daaad3`) |

## Знахідки та рішення

| # | Sev | Файл | Суть | Рішення |
|---|---|---|---|---|
| 1 | MAJOR | orders.rs `apply_full_refund` | Ніщо не закриває `refund_jobs`: коли провайдер сам повернув гроші (`charge.refunded`), борг лишався `pending` — леджер каже «винні», гроші вже повернуто; PAY-11 повернув би вдруге | **Прийнято.** `apply_full_refund` ставить `refund_jobs.status='done'`. Плюс (b): `refund_due` ставить статус лише коли борг НОВИЙ (`queue_refund → bool`); повторна подія оплати на записаний борг → `AlreadyHandled`, рішення адміна не відкочується. Тести: розширений `a_payment_on_a_superseded_checkout_is_refunded_not_ticketed`, новий `a_repeat_payment_on_a_recorded_debt_is_already_handled` |
| 2 | MAJOR | orders.rs checkout; spec BR-024 | Два одночасні checkout одного покупця → два холди (supersede бачить лише закомічені pending); spec щойно пообіцяв «ніколи двох одночасних холдів» | **Прийнято.** Міграція 0044: частковий унікальний індекс `orders_one_live_hold_uidx (event_id, buyer_id) WHERE status='pending'` (дублікати закриваються перед побудовою, місця повертаються); checkout мапить unique violation → 409, decrement відкочується. Тест `sec_two_simultaneous_checkouts_of_one_buyer_yield_one_hold` (некомічений холд на одному з'єднанні + справжній checkout на іншому) |
| 3 | MINOR | tests/orders_hold.rs | Тест на legacy-supersede не ловив зайвий `release_seats` — стеля `LEAST(tickets_total, …)` ковтала його (мутант вижив) | **Прийнято.** 3 місця при 2 у продажу — мутант гине |
| 4 | MINOR | webhooks.rs `record_refusal` | Forensic-запис із `?` на шляху запиту: без таблиці Stripe отримував 500 і ретраїв би відхилену доставку 3 доби | **Прийнято.** Best effort: помилка логується, відповідь 200. Тест `a_failing_refusal_trail_never_changes_the_answer_to_stripe` |
| 5 | MINOR | docs/spec.md | Перенумерація BR-015/016 → BR-023/024 знову колізувала з наявними BR-023 (Door-QR) і BR-024 (discover) | **Прийнято.** Остаточно **BR-025/BR-026** (перевірено: серед нових дублікатів немає; передіснуючі дублі BR-013/BR-014 не чіпаємо в цій задачі); UC-004/BR-014/коментарі в коді оновлені |
| 6 | MINOR | orders.rs spawn `cancel_charge` | Жоден тест не доводив, що checkout ДОХОДИТЬ до `cancel_charge` (unit-тести є, wiring — ні) | **Прийнято.** Окремий бінарник `tests/checkout_voids_superseded_session.rs`: реальний re-checkout через роутер, wiremock фіксує рівно один `POST /v1/checkout/sessions/cs_old/expire`. Мовчазний `if let Ok(voider)` замінено на `?` |
| 7 | TRIVIAL | admin.rs | Адмін-рефанд expired/failed-замовлення показується як `refunded`, хоча гроші не бралися; відрізняє лише `terminal_event_seen='cancelled'`, якого нема в `AdminOrder` | **Відкладено як TRIVIAL** (§4). Трек: показати маркер в `AdminOrder` разом із PAY-11, коли адмінка отримає стан черги повернень |

## Відповіді absence review, які варто пам'ятати

- Множина `terminal_event_seen` = {superseded, cancelled, refunded, disputed}; `_ => AlreadyHandled` у `fulfill` для невідомого значення — «залишити гроші» як дефолт. Свідомо не змінено (рядок недосяжний; 100 %-гейт), зафіксовано як межа: нове значення маркера = обов'язкова нова гілка в `fulfill`.
- Міграції виконуються на старті API (`db.rs`), тож 0043/0044 і код, що на них спирається, їдуть одним деплоєм; вікна між ними немає.
- Подвійного повернення promo-використання немає: обидва шляхи ідуть через `return_holdings`, ключований `rows_affected` перевороту квитка.

## Coverage-гейт

Перший запуск гейту на гілці (на `dae241d`) дав 98.10 % — див. розділ у REVIEW-3. Пакет покриття увійшов у той самий коміт, що й ці
виправлення; результат повторного запуску — у таблиці перевірки звіту.
