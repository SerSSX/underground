# TASK-203 — Escrow trust-бейджі (INIT-TRUST-ESCROW leaf-1)

**Status:** DONE (verified: web browser + Android device)  **Parent:** TASK-199-B  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

## Raw Request
> Довіра = конверсія на маркетплейсі. Зробити обіцянку escrow ВИДИМОЮ у моменти купівлі: «гроші захищені в escrow
> до події, повернення поки не завершилась». Дизайн — через workflow (3 паралельні читачі backend+web+android → синтез).

## Refined Description
**Scope (presentation-only — БЕЗ змін money-seam `escrow.rs` / API; `api_change_needed=false` за дизайном):**
Чесні escrow trust-бейджі («захищено», НЕ «застраховано» — активний провайдер Noop/Test), лише де у клієнта вже є дані
(paid-vs-free, post-purchase `escrow_status`):
- **Web:** event detail (під ціною, paid-only), checkout-модалка (headline над наявним fine-print + /refunds), event-card
  chip «🛡 escrow» (paid-only, 1 слово).
- **Android:** event detail (між ціною і Buy, `!isFree`), checkout `TestPayScreen`, e-ticket (`escrow_status ∈ held|clearing`,
  над dispute-CTA). Зелений `Ok` колір.

**Non-goals / guardrails:** НЕ показувати на free-подіях (нема escrow); НЕ на frozen/refunded/cleared/paid_out; НЕ чіпати
`escrow.rs`/orders/money; зберегти детальний checkout-параграф + /refunds. (Workflow-risks дотримано.)

**Impacted:** без нових BR/UC/міграцій/spec/API. Presentation-only.

## Verification
- **Web (browser):** paid LOUNA → pre-purchase badge ✓; **free event → БЕЗ badge** ✓ (чесно); paid cards → «🛡 escrow» chip ✓; 0 console-помилок.
- **Android (device):** paid «Rounding Test» detail → «🛡 Захищена покупка…» ✓; checkout → «🛡 Гроші захищені…» ✓;
  e-ticket (escrow held) → «🛡 Кошти захищені в escrow до завершення події» ✓. compile+unit ✓.
- **Дизайн:** workflow `trust-escrow-design` (3 explore + 1 opus-synthesis, 4 агенти, 207k tokens).
