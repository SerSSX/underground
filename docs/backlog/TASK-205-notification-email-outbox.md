# TASK-205 — Email-доставка retention-нотифікацій (INIT-LIFECYCLE-TRIGGERS leaf-2)

**Status:** DONE (100% coverage + integration + migration applied)  **Parent:** TASK-199-D  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

## Raw Request
> Замкнути retention-канал ПОЗА застосунком: `event_reminder`+`review_prompt` мають іти й на email через наявний
> email-outbox seam (`email.rs` + worker у `main.rs`), як order-confirmation.

## Refined Description
**Scope (additive; на наявній email-інфрі; SendGrid env-gated → лог `email.deliver` як для order-email):**
- **Міграція 0038:** `notifications.email_sent BOOLEAN NOT NULL DEFAULT false` + **backfill існуючих = true** (щоб перший
  деплой НЕ розіслав історичний backlog).
- **`email.rs`:** `PendingNotificationEmail` + `pending_notifications` (лише `blocked=false` + allow-list kinds
  `event_reminder|review_prompt` — НЕ шумні match/organizer) + `mark_notifications_sent` (watermark, ідемпотентно) +
  `build_notification_email` (subject=title, body=notification-body + CTA).
- **`main.rs email_worker`:** тепер зливає ДВА outbox-и (order + notification), без short-circuit на порожньому order-батчі.

**Non-goals (окремі листя):** email-preference toggle (opt-out UI); digest-агрегація; throttle-gate; реальний SendGrid
HTTP-транспорт (лишається env-gated stub, як був для order-email); більше emailed-kinds.

**Impacted:** без нових BR/UC/spec/ендпоінтів. 1 additive міграція + notification `email_sent`.

## Verification
- Unit `email.rs`: `notification_template_uses_body_then_falls_back_to_title`.
- Integration `tests/email.rs`: `notification_outbox_emails_retention_kinds_once` — лише retention-kind emailed;
  noisy `event_match` виключено; **[SEC] blocked user не emailed**; watermark → не re-selected.
- **fmt + clippy(-D) + 100% coverage** (email.rs 107/107, TOTAL 4582/4582). Міграція 0038 застосована на dev-DB (колонка є).
