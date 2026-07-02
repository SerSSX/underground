# TASK-204 — Post-event review-prompt lifecycle trigger (INIT-LIFECYCLE-TRIGGERS leaf-1)

**Status:** DONE (100% coverage + integration-verified)  **Parent:** TASK-199-D  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

## Raw Request
> Retention-петля: після події, яку відвідав, — нагадування «залиш відгук». Драйвить review-контент → довіру →
> конверсію. На наявному notification/worker seam (як `event_reminder`).

## Refined Description
**Scope (additive; на наявній інфрі; без нових ендпоінтів/схеми):**
- `reminders::review_prompt_sweep(db, recency_days)` — для кожної **published-події, що завершилась** у межах
  `recency_days` (`COALESCE(ends_at, starts_at+3h) < now()` і в вікні), вставляє `review_prompt`-notification кожному
  тримачу квитка (`active|used`), хто **ще не отримував prompt** і **ще не залишив відгук** (NOT EXISTS-dedup, без ledger);
  локалізовано (uk/en). Ідемпотентно.
- Заведено у `main.rs reminder_worker` (той самий tick, `REVIEW_PROMPT_RECENCY_DAYS=14`).
- Доставка: у наявну `notifications` → клієнти показують у «Вхідні» (`GET /api/me/notifications`); подіє-лінк через `event_id`.

**Non-goals (окремі листя INIT-LIFECYCLE-TRIGGERS):** throttle-gate (глобальний ліміт нотифікацій/вікно); digest;
email-outbox доставка цих нотифікацій; інші тригери (sold-out, price-drop). Наявні `event_reminder`/`event_match`/
`organizer_event` вже є.

**Impacted:** без нових BR/UC/spec/API/міграцій. Новий notification `kind='review_prompt'`.

## Verification
- Тести `tests/reminders.rs`: `review_prompt_sweep_prompts_attendees_once` (prompt once + idempotent + non-attendee excluded
  + en-locale body) + `review_prompt_sweep_skips_reviewed_and_stale` ([SEC]/correctness: already-reviewed + поза recency-вікном → 0).
- **fmt + clippy(-D) + 100% coverage** (reminders.rs 63/63, TOTAL 4540/4540). Верифікація через client-facing `/api/me/notifications`.
