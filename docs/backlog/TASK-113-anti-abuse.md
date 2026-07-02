# TASK-113 — Anti-abuse контур навколо guest/gift-claim

**Status:** IN PROGRESS (core rate-limit DONE, verified)

> **Реалізовано (verified `cargo test`):** reusable DB-backed fixed-window rate limiter —
> міграція `0036_rate_events.sql`; `api/src/ratelimit.rs::hit(bucket, max, window)`; `AppError::RateLimited` → 429;
> вбудовано в `auth::login` (10/5хв на email, BR-017) і `emailcode::verify_code` (10/10хв — захист від brute-force коду).
> 2 integration [SEC]-тести (login+verify throttle) + 0 регресій; повний суіт 233 passed / 0 failed.
> **Залишок (LOW-листя):** claim/gift rate-limit + device/email-cap на масовий guest-claim (вмикається разом із TASK-114);
> single-use short-TTL signed token для claim/gift уже є (TASK-110, BR-022).
**Created:** 2026-06-29  **Updated:** 2026-06-29

## Raw Request

> «Anti-abuse контур навколо guest/gift-claim» (ініціатива INIT-ANTIABUSE, Phase 1).
> Умова готовності віральних фіч: single-use short-TTL signed token, rate-limit
> (login/checkout/claim), device/email cap, базовий velocity/blacklist. [SEC]-важлива
> задача. Декомпозуй (token, rate-limit, caps). Фічу НЕ вмикати без контуру.

## Refined Description

**Scope:**
- Окремий signed **claim-token seam** (`api/src/claims.rs`): HMAC-SHA256 над canonical payload
  `{ticket_id, gift_id, exp, nonce}`, секрет `CLAIM_TOKEN_SECRET` — ОКРЕМИЙ від `JWT_SECRET` і
  від QR-токена; single-use (атомарне споживання у claim-транзакції); short-TTL (`CLAIM_TTL_HOURS`,
  default 72h); auto-revert невибраного подарунка до дарувальника фоновим reaper-джобом.
- **Rate-limit** на `login` / `checkout(reserve)` / `claim` / `send-code` / `gift-create`:
  per-IP і per-email fixed-window лічильник у Postgres (`claim_rate_events`), без Redis.
  Наявний `AUTH_LOGIN_RATE_LIMIT` (BR-017) переиспользовується для login-шляху.
- **device/email-cap** на FREE-claim: N free-claims на email за rolling-вікно і на `device_hash`
  (з наявного `X-Device-Id` / `devices.rs`) за вікно; paid не капається.
- **magic-code** підтвердження власності: 6-значний код у листі отримувачу (окремо від лінку),
  hashed у БД (`HMAC-SHA256(code, CLAIM_TOKEN_SECRET)`), `code_attempts`-cap; guest без коду не
  завершує claim (захист від крадіжки по відкритому першим лінку).
- **Базовий velocity/blacklist:** rolling-velocity по `email`/`device_hash`/`IP` bucket-key поверх
  `claim_rate_events`; денилист (`abuse_blocklist` за email/device/IP) з інлайн-превенцією у claim/reserve.
- **Feature-flag** `GIFT_CLAIM_ENABLED` (env, default false): усі нові endpoint-и 404 поки OFF.
- Tracking-події через наявний `telemetry.rs::log` seam (K-factor вимірюваний).

**Non-goals:**
- Будь-яка грошова/escrow-логіка, реальний PSP, payout (product-first, payment last) — контур
  торкається лише unclaimed free/RSVP-квитків через `NoopProvider`.
- Ротація QR-токена на вході (окрема задача rotating-QR) — інший секрет, інший потік.
- CAPTCHA / зовнішній anti-bot вендор, distributed rate-limit сховище (Redis),
  fraud-scoring ML / risk-engine, phone/SMS-верифікація — поза MVP контуру (лишаємо гачки).

**Impacted UCs:** new UC required (guest/gift-claim anti-abuse) — див. **TASK-101 spec reconciliation**
(станів `unclaimed`/`gift-claim` ще нема в `docs/spec.md`). Дотичні наявні: UC-Auth (login rate-limit, BR-017).

**Impacted BR/WF:** BR-006 (гроші лише через `escrow.rs` — контур їх НЕ торкається, інваріант зберігається),
BR-017 (login rate-limited). Нові нормативні правила (single-use short-TTL token + rate-limit +
device/email-cap як умова готовності frictionless-петель) фіксуються у spec ПЕРЕД кодом.

**Dependencies:**
- **TASK-101** — spec reconciliation: внести стан `unclaimed`, gift/claim-token lifecycle, anti-abuse
  умову готовності та email-magic-code-виняток у `docs/spec.md` (rules §0/§2) ПЕРЕД кодом.
- **INIT-TICKET-UNCLAIMED** — вводить `unclaimed`-стан квитка (`owner_id` nullable/holder); hard
  dependency: без нього claim не має що привласнювати (координувати міграцію `tickets`).
- Наявні seam-и (розширюються, НЕ переписуються): `telemetry.rs::log`, `email.rs` + `0026_email_outbox`,
  `devices.rs`/`X-Device-Id`, `config.rs` fail-fast validate, `escrow.rs`/`NoopProvider`.
- Дотично: INIT-CHANNELS (доставка magic-code листа), INIT-GIFTLINK/INIT-BROWSE-AUTH (споживають seam).

**Risks / Open Questions:**
- Postgres fixed-window rate-limit під навантаженням — прийнятно для першого релізу; Redis відкладено.
- Значення cap/TTL/velocity-порогів (`free_claim_email_cap`, `free_claim_device_cap`, вікно) —
  дефолти в env, тюнинг після виміру fraud-loss.
- `device_hash` спуфиться клієнтом — cap не єдиний бар'єр; тримається у парі з email magic-code + IP-velocity.
- Blocklist governance (хто додає/знімає) — на старті лише admin-ручний seam; авто-freeze поза scope.

## Estimation

**Level:** MEDIUM  **Justification:** один новий модуль `api/src/claims.rs` + 3 міграції + reaper-джоб
поверх наявних seam-ів; клієнтські стани (Web/Android) — тонкі екрани claim; без нових грошових шляхів.
Проте задача перетинає 4 шари (API/Web/Android/Admin) і несе широкий SEC-контур, тому за rules §12.5
MEDIUM позначено flag+confirm і РОЗБИТО на LOW-листя нижче — код починається лише після того, як усі
листя LOW і всі залежності (TASK-101, INIT-TICKET-UNCLAIMED міграція) готові.

## Планова декомпозиція (LOW-листя — обовʼязково перед кодом, rules §12.5/§12.6)

Межі декомпозиції: distinct layer (API/Web/Android/Admin/Infra) + distinct concern (token / rate-limit / caps),
sequential phases (config→schema→token→rate-limit→caps→endpoints→reaper→clients→admin).

| Sub-task ID | Title | Estimation | Status | Depends On |
|---|---|---|---|---|
| TASK-113-A | Infra: config keys + `GIFT_CLAIM_ENABLED` flag + fail-fast `validate()` (`config.rs`) | VERY-LOW | READY | TASK-101 |
| TASK-113-B | Schema: міграції `0033_gift_claims`, `0034_claim_ratelimit`, `abuse_blocklist` (+ індекси) | LOW | READY | TASK-113-A, INIT-TICKET-UNCLAIMED |
| TASK-113-C | Token: `api/src/claims.rs` HMAC-SHA256 sign/verify, nonce, TTL, окремий секрет | LOW | READY | TASK-113-A, TASK-113-B |
| TASK-113-D | Rate-limit seam: Postgres fixed-window (`claim_rate_events`, ON CONFLICT DO UPDATE RETURNING) | LOW | READY | TASK-113-B |
| TASK-113-E | Caps + velocity + blocklist: device/email free-cap, rolling-velocity, inline blocklist-check | LOW | READY | TASK-113-B, TASK-113-D |
| TASK-113-F | Magic-code: mint 6-цифр, hashed, `code_attempts`-cap, send-code endpoint (email seam) | LOW | READY | TASK-113-C, INIT-CHANNELS |
| TASK-113-G | API endpoints: `POST /gift`, `GET /claims/:token`, `send-code`, `claim`, `reserve` guard | LOW | READY | TASK-113-C, TASK-113-D, TASK-113-E, TASK-113-F |
| TASK-113-H | Reaper job: auto-revert pending gift по TTL (у `reminders`/`notifications` loop, атомарний CAS) | LOW | READY | TASK-113-B, TASK-113-C |
| TASK-113-I | Admin (read-only): таблиця `gift_claims` + дії Revert / Reset-cap / Blocklist (`admin/app/**`) | LOW | READY | TASK-113-G |
| TASK-113-J | Web claim-лендінг `/claim/[token]` + guest e-ticket `/t/[token]` (стани expired/claimed/cap/code/signup) | LOW | READY | TASK-113-G |
| TASK-113-K | Android claim-екран (deep-link `ug://claim/{token}`) + «Подарувати»-діалог + сегмент «Подаровані» | LOW | READY | TASK-113-G |
| TASK-113-L | Telemetry: 8 tracking-подій контуру через `telemetry.rs::log` (K-factor) | VERY-LOW | READY | TASK-113-G |

## Execution Order

1. TASK-101 (spec) + INIT-TICKET-UNCLAIMED (unclaimed-міграція) — hard prerequisites.
2. TASK-113-A → TASK-113-B (config, schema).
3. TASK-113-C, TASK-113-D паралельно (token seam, rate-limit seam).
4. TASK-113-E, TASK-113-F (caps/velocity/blocklist, magic-code).
5. TASK-113-G (endpoints, зшивання) → TASK-113-H (reaper).
6. TASK-113-J, TASK-113-K, TASK-113-I паралельно (Web, Android, Admin) → TASK-113-L (telemetry) наскрізно.
7. Увімкнути `GIFT_CLAIM_ENABLED=true` ЛИШЕ коли весь контур + SEC-матриця зелені.

## Speck Reference

docs/specks/SDD-113-anti-abuse.md
