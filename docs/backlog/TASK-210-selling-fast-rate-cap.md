# TASK-210 — Per-user rate cap on "selling fast" nudges (INIT-LIFECYCLE-TRIGGERS leaf-3, throttle precursor)

**Status:** DONE (100% line coverage + integration-verified)  **Parent:** TASK-199-D  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

## Raw Request
> Захистити конверсійні тригери від spam-firehose: користувач, що зберіг багато подій, не має отримувати
> необмежену кількість `selling_fast`-нуджів за один tick / вікно. Перший крок до глобального throttle-gate (TASK-199-D).

## Refined Description
**Проблема:** `selling_fast_sweep` (TASK-209) вставляє нудж кожному saver-у кожної low-stock події одним
`INSERT ... SELECT`. Saver, що зберіг 10 подій, які всі просіли по стоку в один tick → 10 нотифікацій одразу.
`event_reminder`/`review_prompt` природно обмежені (одна подія → максимум одна), а `selling_fast` — ні.

**Scope (additive; на наявній інфрі; без нових ендпоінтів/схеми):**
- `selling_fast_sweep(db, threshold_frac, **daily_cap**)` — нова сигнатура. Свіжі кандидати (після per-(user,event)
  dedup + holder-guard) ранжуються `row_number() OVER (PARTITION BY user_id ...)`; вставляються лише поки
  `rn <= daily_cap − (кількість selling_fast за останні 24h)`. Це кап **всередині одного statement** І **між
  послідовними statement-ами** (лічильник бачить закомічені рядки попередніх tick-ів у 24h-вікні).
- `main.rs`: константа `SELLING_FAST_DAILY_CAP: i64 = 2`, передається у sweep.
- `event_reminder`/`review_prompt` **не** зачеплені (event-anchored, вже bounded).

**Non-goals (окремі листя):** повний cross-trigger throttle-gate (спільний бюджет по всіх промо-kind-ах — коли
з'явиться 2-й промо-тригер, напр. price_drop/sold_out); digest; per-event налаштування капу; global quiet-hours.

**Impacted:** без нових BR/UC/API/міграцій. Змінено лише внутрішню сигнатуру `selling_fast_sweep` (не HTTP-API).

## Verification
- Тести `tests/reminders.rs`: `selling_fast_sweep_nudges_low_stock_savers` (оновлено під нову сигнатуру, cap=5 →
  поведінка незмінна) + новий `selling_fast_sweep_respects_daily_cap` — saver зберіг 3 low-stock події, cap=2 →
  1-й прохід вставляє рівно 2 (не 3); повторний прохід у тому ж вікні → 0 (бюджет вичерпано), хоча 3-тя подія
  досі eligible.
- **fmt + clippy(-D) + 100% line coverage**. Верифікація через client-facing `/api/me/notifications`.
