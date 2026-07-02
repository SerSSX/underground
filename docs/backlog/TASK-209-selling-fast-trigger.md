# TASK-209 — "Selling fast" scarcity/conversion trigger (INIT-LIFECYCLE-TRIGGERS leaf-2)

**Status:** DONE (100% line coverage + integration-verified)  **Parent:** TASK-199-D  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

## Raw Request
> Конверсійна петля: коли на збереженій події лишається мало квитків — нудж «встигни купити». Scarcity → urgency →
> конверсія збережень у покупки. На наявному notification/worker seam (як `event_reminder`/`review_prompt`).

## Refined Description
**Scope (additive; на наявній інфрі; без нових ендпоінтів/схеми):**
- `reminders::selling_fast_sweep(db, threshold_frac)` — для кожної **published, on-sale** події (`sales_closed=false`,
  не в минулому), чий залишок впав до `threshold_frac` від `tickets_total` (але **не sold-out**: `tickets_left>0`),
  вставляє `selling_fast`-notification кожному, хто **зберіг** подію (`event_saves`) і **ще не тримає квиток**
  (`tickets.status IN ('active','used')`). Раз на (user,event) через NOT EXISTS-dedup (без ledger); локалізовано (uk/en).
- Заведено у `main.rs reminder_worker` (той самий tick, `SELLING_FAST_THRESHOLD=0.15`).
- Доставка: у наявну `notifications` → «Вхідні» (`GET /api/me/notifications`); подіє-лінк через `event_id`. Kind
  `selling_fast` додано в retention-email allow-list (`email::pending_notifications`) → outbox доставляє його як
  `event_reminder`/`review_prompt` (TASK-205).

**Чому саме savers, не ticket-holders:** нудж має конвертувати намір → покупку. Хто вже тримає квиток — не таргет
(виключено), інакше це спам. Sold-out (`tickets_left=0`) не тригерить — нема що продавати.

**Non-goals (окремі листя INIT-LIFECYCLE-TRIGGERS):** глобальний throttle-gate; digest; price-drop / back-in-stock
тригери; per-organizer opt-out. Поріг — константа (не per-event налаштування).

**Impacted:** без нових BR/UC/API/міграцій. Новий notification `kind='selling_fast'`; повторно використано
`notification.email_sent` (міграція 0038, TASK-205).

## Verification
- Тести `tests/reminders.rs`: `selling_fast_sweep_nudges_low_stock_savers` — нудж лише saver-у low-stock події
  (ticket-holder виключено), ідемпотентно (2-й прохід → 0), well-stocked подія ніколи не тригерить, holder без нуджа.
- `tests/email.rs`: `notification_outbox_emails_retention_kinds_once` — `selling_fast` разом із `review_prompt`
  потрапляє в email-outbox, шумний `event_match` — ні; blocked-user не отримує ([SEC]).
- **fmt + clippy(-D) + 100% line coverage** (reminders.rs 94/94 lines, TOTAL 4613/4613). Верифікація через
  client-facing `/api/me/notifications`.
