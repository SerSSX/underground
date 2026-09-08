# REVIEW-3 — Третє незалежне рев'ю (мутаційна перевірка REVIEW-2 + absence review) і його виправлення

| Поле | Значення |
|---|---|
| **Тип** | Review record + fix task |
| **Status** | `IN PROGRESS` — виправлення в коді; четверте рев'ю (підтвердження закриття) ще не проведене |
| **Дата рев'ю** | 2026-09-09 |
| **Рев'юер** | окремий агент (opus); мутував кожен із 11 заявлених контролів REVIEW-2, писав власні проби (R3-A…D), ганяв 5 бінарників |
| **Вердикт рев'ю** | **мерджити не можна**: 1 CRITICAL, 2 MAJOR, 6 MINOR, 1 TRIVIAL; 10/11 мутантів убито, m8 вижив |
| **Ревʼюване** | `425449d`, `9daaad3` (база `57250d6`) |

## Знахідки та рішення

| # | Sev | Файл | Суть | Рішення |
|---|---|---|---|---|
| 1 | CRITICAL | admin.rs | Адмін-рефанд PENDING-замовлення ставив `status='refunded'` без `terminal_event_seen` і без `refund_jobs`; пізня оплата → `AlreadyHandled`: гроші є, квитка нема, боргу на обліку нема | **Прийнято.** Маркер `cancelled` (гроші не бралися) → пізня оплата через `fulfill` стає `refund_due` з рядком у `refund_jobs`; для оплаченого — маркер `refunded` + рядок у `refund_jobs` одразу (борг фіксується, PAY-11 виконує). Тести: розширений `admin_refund_of_a_pending_order_returns_its_seat`, новий `admin_refund_of_a_paid_order_records_the_debt_once` |
| 2 | MAJOR | orders.rs `apply_full_refund` | Виправлення N4 (місце pending-замовлення) було лише в admin.rs; вебхук `charge.refunded`, що обігнав `paid`, назавжди губив місце | **Прийнято.** Одна функція `orders::return_holdings` для обох шляхів (квиток+місце+промо або місце холду). Тест: розширений `a_refund_that_arrives_before_the_payment_blocks_the_late_payment` (tickets_left 2) |
| 3 | MAJOR | webhooks.rs | `note_webhook_rejected` (UPDATE рядка `payment_settings`) виконувався ДО `ratelimit::hit` — анонімний флуд без ліміту писав у той самий рядок, що й верифіковані доставки | **Прийнято.** Спочатку троттл, потім лічильник. Тест: флуд-тест перевіряє `stripe_webhook_rejections == 0` після 429 |
| 4 | MINOR | orders.rs checkout | `cancel_charge` чекався у запиті покупця (до 20 с таймауту провайдера на best-effort роботу) | **Прийнято.** `tokio::spawn` з окремим `resolve(&settings)`; маркер на замовленні — справжній контроль. `checkout_provider_down` ганяє шлях із мертвим провайдером |
| 5 | MINOR | orders.rs `fulfill` | Правило «superseded ніколи не квиток» жило лише у вебхуку; `fulfill` (pub) мінтив | **Прийнято.** `fulfill` читає `terminal_event_seen`: `superseded`/`cancelled` → `refund_due`; `refunded`/`disputed` → `AlreadyHandled` (провайдер уже повертає — без другого повернення). Вебхук лише логує і пропускає далі (тепер ще й зберігає PaymentIntent для рефанду). Тест `a_superseded_order_paid_through_fulfill_directly_is_refunded_not_ticketed` |
| 6 | MINOR | orders.rs checkout | Legacy pending без холду не супersede-иться і не протухає — платний назавжди, два місця на покупця можливі | **Прийнято.** Supersede бере всі pending покупця, місце повертає лише за тими, що мали холд; міграція 0043 закриває наявні legacy pending як `expired`. Тест `a_legacy_hold_less_pending_order_is_superseded_without_releasing_a_seat` |
| 7 | MINOR | webhooks.rs | Після m3 відхилена по біндингу доставка не лишала жодного запису | **Прийнято.** Таблиця `webhook_refusals` (0043), запис окремим з'єднанням після rollback. Тест: розширений `an_ignored_delivery_does_not_burn_its_event_id` |
| 8 | MINOR | webhooks.rs LS | POST без `x-signature` → 401 до лімітера, кожен коштує SELECT | **Прийнято.** Відсутній заголовок = відхилена доставка, метриться. Тест `an_unsigned_ls_delivery_is_metered_like_a_bad_signature` |
| 9 | TRIVIAL | ratelimit.rs | `sweep` без індексу по `created_at` | **Прийнято.** `rate_events_created_idx` (0043) |
| 10 | MINOR | REVIEW-2 m4/m8 | Два прийняті виправлення без контроль-присутнього тесту (m8 мутант вижив) | **Прийнято.** `live_mode_with_a_money_free_provider_is_refused_at_save` (обидва порядки полів), `stripe_key_kind_reads_secret_and_restricted_keys` |

## Spec conflicts (усі виправлені в цьому ж коміті)

- Нові BR-015/BR-016 колізували з наявними → перенумеровані в **BR-023/BR-024**; абзац про QR/`event_staff` повернуто під BR-012.
- UC-004 казав «`paid` + ескроу `refund_due`» — виправлено на окремий статус `refund_due`; додано посилання на BR-023/BR-024.
- BR-024 більше не стверджує абсолют «ніколи двох місць»: один покупець не тримає двох ОДНОЧАСНИХ холдів; послідовні купівлі — окремі замовлення.
- BR-014 доповнено: закрите до оплати (`superseded`/`cancelled`) → `refund_due`; закрите провайдером (`refunded`/`disputed`) → без другого повернення; адмін-рефанд оплаченого → рядок у `refund_jobs`.
- Коментар у застосованій міграції 0041 про словник статусів застарів — не редагуємо застосовану міграцію; словник тепер описаний у BR-014/BR-023.

## Процесні зауваги рев'ю

- Обидві зміни ассертів у 9daaad3 визнані **легітимними наслідками** виправлень (N3, m3), не послабленням; тестів не видалено (272 → 282).
- REVIEW-1 переведено в `DONE` автором за вердиктом REVIEW-2 — тут статус фіксує вердикт рев'юера, а не самоприймання; те саме для REVIEW-2 за вердиктом REVIEW-3.
- Відкрите рішення власника (N5 з REVIEW-2, тест `error_paths` у 57250d6) лишається відкритим.
