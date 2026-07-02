# SDD-113 — Anti-abuse контур для guest/gift-claim (single-use short-TTL token + rate-limit + device/email-cap + velocity/blocklist + magic-code)

**Impacted UCs:** new UC required (guest/gift-claim anti-abuse) — див. TASK-101 spec reconciliation; дотично UC-Auth (login rate-limit).
**Impacted BR/WF:** BR-006 (гроші лише через `escrow.rs` — контур НЕ торкається коштів), BR-017 (login rate-limited); нові умови готовності frictionless-петель фіксуються у `docs/spec.md` ПЕРЕД кодом.

## Scope / Non-goals

**Scope.** Обгорнути весь frictionless guest/gift-claim шлях мінімальним, але повним anti-abuse контуром:
- Окремий signed claim-token seam (`api/src/claims.rs`): HMAC-SHA256 над `{ticket_id, gift_id, exp, nonce}`, секрет `CLAIM_TOKEN_SECRET` (окремий від `JWT_SECRET` і QR); single-use; short-TTL (`CLAIM_TTL_HOURS`, default 72h); auto-revert по TTL фоновим reaper (в `reminders`/`notifications` loop).
- Rate-limit per-IP і per-email fixed-window у Postgres (`claim_rate_events`) на `login`/`reserve`/`claim`/`send-code`/`gift-create`.
- device/email-cap на FREE-claim (`device_hash` з `X-Device-Id`/`devices.rs`); paid не капається.
- Базовий velocity (rolling-count per bucket) + `abuse_blocklist` (email/device/IP) з inline-превенцією.
- magic-code (6 цифр, hashed, `code_attempts`-cap) як підтвердження власності; лінк без коду не завершує guest-claim.
- Feature-flag `GIFT_CLAIM_ENABLED` (default false): усі нові endpoint-и 404 поки OFF.
- Tracking через наявний `telemetry.rs::log`.
- Модулі: `api/src/claims.rs` (+ міграції), `api/src/config.rs`, `api/src/reminders.rs` (reaper), `web/app/claim/[token]/*`, `web/app/t/[token]/*`, `android/.../ui/claim/**`, `admin/app/claims/**`; escrow seam лишається provider-neutral і незайманим.

**Non-goals.** Грошова/escrow-логіка, реальний PSP, payout; rotating-QR; CAPTCHA/anti-bot вендор; Redis; fraud-scoring ML; phone/SMS. Blocklist auto-freeze — поза MVP (лише ручний admin-seam).

## Acceptance Criteria

- **AC-1:** Валідний claim-token + правильний magic-code для `pending`-подарунка привласнює квиток отримувачу (`status->claimed`), повертає e-ticket; повторний claim того ж token → `410`.
- **AC-2:** claim-token підписаний ОКРЕМИМ `CLAIM_TOKEN_SECRET`: токен, підписаний `JWT_SECRET`, відхиляється; підробка payload (зміна `ticket_id`) → `401`.
- **AC-3:** Прострочений claim-token (`exp < now`) → `410`; reaper переводить pending-gift у `reverted` після `expires_at` і повертає квиток дарувальнику (`owner_id=sender`).
- **AC-4:** Guest без акаунта завершує claim ЛИШЕ за наявності magic-code, надісланого на `recipient_email`; лінк без коду → стан `need-code`, квиток НЕ виданий.
- **AC-5:** Free-claim капається: понад `free_claim_email_cap` на email АБО `free_claim_device_cap` на `device_hash` у вікні → `429 cap_blocked` + telemetry `cap_blocked`.
- **AC-6:** Rate-limit per-IP і per-email на `login`/`reserve`/`claim`/`send-code`/`gift`: понад ліміт → `429 ratelimit_blocked`; лічильник атомарний (`INSERT ... ON CONFLICT DO UPDATE ... RETURNING`).
- **AC-7:** Базовий velocity/blocklist: email/device/IP у `abuse_blocklist` → claim/reserve одразу `403 blocked`; rolling-velocity понад поріг у вікні → `429`, обидва трекаються.
- **AC-8:** При claim фіксується `claimed_device_hash`; дарувальник і власник бачать «забрано з іншого пристрою» (масковано); спроба з іншого пристрою на claimed-gift → `410`.
- **AC-9:** `GIFT_CLAIM_ENABLED=false` → усі claim-endpoint-и `404`; фіча не активна, поки контур не увімкнено.
- **AC-10:** magic-code: рівно 6 цифр, у БД лише HMAC-хеш; `code_attempts` обмежені (5) → блок з вимогою re-send; сирий код ніколи не логується.
- **AC-11:** claim не мінтить нові квитки — лише привласнює наявний unclaimed-квиток; sold-out подія не інфлейтить seat-count (assert незмінний при gift/claim/revert).
- **AC-12:** Кожен крок емітить telemetry-подію (`gift_created`, `claim_link_opened`, `claim_code_sent`, `claim_succeeded`, `claim_failed{reason}`, `claim_reverted`, `ratelimit_blocked`, `cap_blocked`) — K-factor вимірюваний.

## Security Acceptance Criteria

- **SEC-1 (Token brute-force):** claim-token = `ticket_id+gift_id+exp+128-bit nonce` під HMAC-SHA256; перебір nonce обчислювально неможливий; невалідний токен → `401` без витоку існування gift.
- **SEC-2 (Replay / double-claim):** single-use через `UNIQUE(ticket_id) WHERE status='pending'` + атомарний CAS (`UPDATE ... WHERE id=$ AND status='pending' RETURNING`) у тій самій транзакції, що `owner_id`; перегони → рівно один claim, інший `410`.
- **SEC-3 (Authz / IDOR):** `gift-remind`/`revert`/`gift-create` дозволені лише `sender`/owner; чужий `ticket_id`/`gift_id` → `403`; `GET /claims/:token` публічний, але не розкриває owner/PII третіх осіб.
- **SEC-4 (Claim-after-sold-out / seat inflation):** claim не викликає `decrement_tickets` і не мінтить; лише переносить `owner_id` — місця не інфлейтяться (SEC-assert seat-count).
- **SEC-5 (Botnet free-farm):** device/email-cap + per-IP rate-limit + velocity + magic-code-ownership спільно ламають масовий фарм; `cap_blocked`/`ratelimit_blocked` трекаються для алертів.
- **SEC-6 (Gift-theft по відкритому першим лінку):** magic-code на `recipient_email` обов'язковий для guest — володіння лінком без пошти не завершує claim.
- **SEC-7 (Secret-substitution / algorithm confusion):** HMAC-SHA256 pinned, окремий секрет від JWT; токен під `JWT_SECRET` або іншим алго → `401`.
- **SEC-8 (Magic-code brute):** 6-значний код + `code_attempts`-cap (5) + rate-limit на `send-code`; хеш у БД, constant-time порівняння.
- **SEC-9 (Enumeration / timing leak):** `GET /claims/:token` дає ідентичну generic-форму (`410`) для invalid/expired/claimed; `send-code` не підтверджує існування email.
- **SEC-10 (Oversized / injection):** `code`/`email`/`device_id`/`message` валідуються за довжиною й форматом (bounded body); параметризовані SQLx-запити — жодної конкатенації; oversized body → `413`/`400`.
- **SEC-11 (PII/секрети в логах):** `recipient_email` маскується; сирий magic-code і claim-token ніколи не логуються (лише `gift_id`/nonce-hash у telemetry).
- **SEC-12 (Auto-revert race):** reaper і ручний claim конкурують за той самий CAS; програвший → `410`; квиток не опиняється у двох власників.
- **SEC-13 (Config fail-fast):** production `Config.validate()` падає, якщо `CLAIM_TOKEN_SECRET` = dev-default, `< 32 байти`, або `== JWT_SECRET`.

## Failure Modes / Error Mapping

| Умова | HTTP | Тіло / стан |
|---|---|---|
| Feature OFF (`GIFT_CLAIM_ENABLED=false`) | `404` | generic not-found (фіча прихована) |
| Невалідна підпис/payload/алго токена | `401` | `invalid_token` (без розкриття існування) |
| Expired / already-claimed / reverted token | `410` | generic `gone` (однакова форма — anti-enumeration) |
| Дублікат pending-gift на квиток | `409` | `gift_already_pending` |
| Rate-limit / velocity перевищено | `429` | `ratelimit_blocked` (+ `Retry-After`) |
| Free device/email-cap перевищено | `429` | `cap_blocked` |
| Email/device/IP у blocklist | `403` | `blocked` |
| Guest без magic-code | `200` | стан `need-code` (квиток НЕ виданий) |
| Невірний code / понад `code_attempts` | `422`/`429` | `wrong_code` / `code_locked` |
| Oversized/invalid body | `400`/`413` | `bad_request` |
| Sold-out при claim | `200` | success без seat-inflation (SEC-4) |

## Test Matrix

Стадії: Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack. UI-стадії заповнені лише для AC/SEC з клієнтським екраном (Web `/claim`, Android claim). Кожна сюїта — positive + negative + [SEC] вектори; container-first, 100% unit+integration.

| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 claim+code → claimed, повтор → 410 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-2 окремий секрет / підробка payload | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-3 expired → 410 + reaper revert | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| AC-4 guest без code → need-code | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-5 device/email free-cap → 429 | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| AC-6 rate-limit per-IP/email → 429 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-7 velocity/blocklist → 429/403 | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| AC-8 claimed_device_hash + «інший пристрій» | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| AC-9 flag OFF → усі endpoint 404 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-10 magic-code 6 цифр, hashed, attempts | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| AC-11 no seat-inflation на claim/revert | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-12 telemetry-події контуру | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-1 token brute → 401 no-leak | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-2 concurrent claim → 1 winner | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-3 IDOR revert/remind чужого → 403 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-4 sold-out claim → no inflation | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-5 botnet farm blocked | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-6 gift-theft лінком без коду → відхилено | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| SEC-7 token під JWT_SECRET/іншим алго → 401 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-8 magic-code brute → lock | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| SEC-9 invalid/expired/claimed → generic 410 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-10 oversized/injection body → 400/413 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-11 PII/секрети не в логах | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-12 reaper vs claim race → 1 owner | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-13 config fail-fast (secret) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ | ✅ |
