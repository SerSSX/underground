# TASK-111 — Канали доставки: FCM push + транзакційний email

**Status:** READY
**Created:** 2026-06-29  **Updated:** 2026-06-29

## Raw Request
> «Зробіть канали доставки справжніми. Зараз FCM push (`main.rs` push_worker) і email (`main.rs` email_worker) — TODO-заглушки, які лише логують intent і одразу ставлять watermark. Потрібен реальний FCM push-транспорт (HTTP v1, OAuth2 з service-account key) + транзакційний email для життєвого циклу: claim/gift-magic-code, квиток (mint), нагадування. Provider-neutral, як payment-seam: trait + Noop/Log + реальний провайдер ОСТАННІМ. Без робочих каналів уся gift/claim/retention-петля — no-op, тож це P0 hard-prerequisite.» (INIT-CHANNELS, Phase 1)

## Refined Description
**Scope:**
- **Push:** замінити log-only цикл у `api/src/main.rs` push_worker на реальний `POST https://fcm.googleapis.com/v1/projects/{project_id}/messages:send` з Bearer OAuth2 access-token, мінтованим локально з SA-key (RS256 JWT-assertion → token endpoint, кеш до `exp-60s`). Per-token результат замість сліпого batch-mark; retry/backoff + dead-letter; drop stale device-token при `UNREGISTERED/NOT_FOUND`.
- **Email seam:** provider-neutral `trait EmailChannel { send(EmailMsg) -> Result<Delivery> }` + `NoopEmail` / `LogEmail` (default, CI-детермінований) / `SendGridEmail`; `EmailChannel::from_env()` резолвить за `EMAIL_PROVIDER`. Дзеркалить payment-provider-registry (`escrow.rs` / `payments.rs`). Реальний провайдер — LAST.
- **Шаблони:** розширити `email::build_confirmation` набором `build_ticket` / `build_receipt` / `build_refund` / `build_guest_magic_code`; email-outbox з одного `kind` на transactional-типи (`ticket`, `receipt`, `refund`, `guest_magic_code`, `order_confirmation`).
- **Integration:** email_worker дренить нові типи через seam; доменні події (mint, refund, guest checkout / gift-claim) ставлять рядки в outbox; admin health-check `GET /api/admin/channels/health` + `POST /api/admin/channels/test`; env-каркас + docs.

**Non-goals:** WebSocket/real-time DM; SMS/Viber/Telegram; браузерний web-push; нові фіче-тригери (gift-delivery UX, reminder-scheduling — окремі споживачі каналу, INIT-LIFECYCLE-TRIGGERS); повний notification-preferences UI; складні HTML-шаблони; заміна payment/escrow-seam; реальний PSP (Phase 4).

**Impacted UCs:** new UC required — «UC-Channels: lifecycle-доставка (push+email)». Поведінка (реальна відправка, transactional-типи, dead-letter, health-check) — нова; фіксується у `docs/spec.md` ПЕРЕД кодом (спец-узгодження через TASK-101 spec reconciliation, rules §2.2). Дотичні наявні: UC-001 (guest magic-code як канал входу).

**Impacted BR/WF:** новий WF-Channels (outbox → worker → provider → watermark|retry|dead-letter); успадковує BR opt-out (`users.blocked=false`) і per-channel mute. Розширює Tracking Plan (delivery success rate — telemetry-seam, `api/src/telemetry.rs`).

**Dependencies:**
- TASK-101 (spec reconciliation, product-first / payment-last) — governing spec-апдейт ПЕРЕД кодом.
- INIT-TICKET-UNCLAIMED / TASK-ANTIABUSE — guest_magic_code email споживає signed-token seam; якщо seam ще не готовий, шаблон приймає токен ззовні (транспорт не блокується, claim-flow — залежний споживач).
- Наявні seam-и: `push.rs` (pending/mark_pushed, `FcmConfig`), `email.rs` (pending/mark_sent/build_confirmation, `EmailConfig`), `device_tokens` (0016), `orders.email_sent` (0026), `notifications.pushed_at` (0016), `telemetry.rs`, provider-neutral escrow/payment-registry як референс-патерн.
- `reqwest` + `rustls` — ДОДАТИ в `api/Cargo.toml` (зараз відсутній; `jsonwebtoken=9` уже є, покриває RS256 assertion).
- НЕ залежить від реального PSP — канали доставляють і на Noop/Test-платежах.

**Risks / Open Questions:**
- SA-key як env vs файл-path (SEC-1: читати з path, не інлайнити) — фіксуємо path.
- Backoff/MAX_ATTEMPTS-параметри та вибір «залишити `orders.email_sent` vs мігрувати в `email_outbox`» — обираємо простіший шлях (залишити `email_sent` для order_confirmation, нова `email_outbox`-таблиця лише для нових типів).
- SSRF: token-endpoint і FCM/SendGrid-хости — фіксований allowlist, без redirect-follow.

## Estimation
**Level:** MEDIUM (flagged per rules §12.5).
**Justification:** Один шар (API), без клієнтських змін, поверх наявних outbox-seam-ів (розширення, не переписування). Але MEDIUM торкається двох різних каналів (FCM/email), нового provider-registry, retry/dead-letter стан-машини й admin-endpoint-ів — не VERY-LOW. Rules вимагають декомпозиції у LOW-листя ПЕРЕД кодом → нижче «Планова декомпозиція» (усі листя LOW, у межах одного API-шару, розбиті по каналу/фазі schema→transport→templates→integration).

## Планова декомпозиція (LOW-листя перед кодом, rules §12.5/§12.6)

| Sub-task ID | Title | Layer / Boundary | Estimation | Status | Depends On |
|---|---|---|---|---|---|
| TASK-111-A | Spec+config+schema: UC-Channels у `spec.md`, env-каркас (`FCM_PROJECT_ID`,`FCM_SA_KEY`,`EMAIL_PROVIDER`,`SENDGRID_API_KEY`,`EMAIL_FROM`), міграція `0033_channel_retry.sql` (notifications+orders retry-колонки, `email_outbox`-таблиця, drain-індекси) | API/Infra · schema | LOW | READY | TASK-101 |
| TASK-111-B | FCM транспорт: `mint_fcm_access_token` (RS256 assertion + кеш до exp-60s) + `fcm_send` (HTTP v1, per-token `FcmOutcome`) + retry/backoff/dead-letter + drop stale token; заміна log-only push_worker | API · push transport | LOW | READY | TASK-111-A |
| TASK-111-C | Email seam: `trait EmailChannel` + `NoopEmail`/`LogEmail`/`SendGridEmail` + `from_env()` registry; заміна log-only email_worker на seam-виклик + retry/backoff/last_error | API · email transport | LOW | READY | TASK-111-A |
| TASK-111-D | Transactional-шаблони: `build_ticket`/`build_receipt`/`build_refund`/`build_guest_magic_code` (HTML+text, escape user-полів) поверх `email_outbox` | API · templates | LOW | READY | TASK-111-C |
| TASK-111-E | Integration+health: доменні події (mint/refund/guest-claim) → outbox; `GET /api/admin/channels/health` + `POST /api/admin/channels/test` (admin-guard, rate-limit, SSRF-allowlist); telemetry delivery-events; docs (`.env.example`, `configuration-manual.md`) | API/Admin · integration | LOW | READY | TASK-111-B, TASK-111-D |

## Execution Order
TASK-111-A → (TASK-111-B ∥ TASK-111-C) → TASK-111-D → TASK-111-E.
B і C незалежні після A (різні канали); D потребує email-seam (C); E зшиває обидва канали + health + docs.

## Speck Reference
docs/specks/SDD-111-delivery-channels.md
