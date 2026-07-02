# SDD-111 — Канали доставки: FCM push (HTTP v1) + транзакційний email за єдиним seam

**Impacted UCs:** new UC required — «UC-Channels: lifecycle-доставка (push+email)» (spec-узгодження через TASK-101, rules §2.2); дотична UC-001 (guest magic-code як канал входу)
**Impacted BR/WF:** новий WF-Channels (outbox → worker → provider → watermark | retry | dead-letter); успадковує opt-out BR (`users.blocked=false` + per-channel mute); розширює Tracking Plan (delivery success rate) через `api/src/telemetry.rs`

## Scope / Non-goals
**Scope (surface = API):**
- FCM HTTP v1 send з OAuth2 Bearer (scope `firebase.messaging`), token мінтований локально з SA-key (RS256 JWT-assertion → `https://oauth2.googleapis.com/token`), кешований до `exp-60s`; per-token `FcmOutcome{Delivered|Unregistered|Retryable|Permanent}`; заміна log-only push_worker (`api/src/main.rs`).
- Provider-neutral email-seam: `trait EmailChannel { send(EmailMsg) -> Result<Delivery> }` + `NoopEmail`/`LogEmail`(default)/`SendGridEmail`; `EmailChannel::from_env()` за `EMAIL_PROVIDER`. Дзеркалить payment/escrow provider-registry.
- Transactional-типи: `ticket`, `receipt`, `refund`, `guest_magic_code`, `order_confirmation` через один `email_outbox` seam і один email_worker.
- Retry/backoff + dead-letter для обох outbox; drop stale FCM-token.
- Admin health-check + synchronous test-send.
- Env-каркас + docs; `docs/spec.md` оновлено ПЕРЕД кодом.

**Non-goals:** WebSocket/DM; SMS/Viber/Telegram; браузерний web-push; нові фіче-тригери (scheduling); повний preferences-UI; складні HTML; заміна payment/escrow; реальний PSP (Phase 4). Escrow-seam (`api/src/escrow.rs`) — недоторканий; гроші не рухаються.

## Acceptance Criteria
- **AC-1:** За заданих `FCM_PROJECT_ID`+`FCM_SA_KEY` push_worker для кожного `PendingPush` робить реальний `POST fcm.googleapis.com/v1/projects/{id}/messages:send` з валідним Bearer; 2xx → `notifications.pushed_at=now()`; при помилці token НЕ помічається доставленим.
- **AC-2:** OAuth2 access-token мінтується з SA-key (RS256 assertion) і кешується; повторні send у межах TTL не роблять зайвого token-запиту; протухлий (`>exp-60s`) — оновлюється.
- **AC-3:** FCM `UNREGISTERED`/`NOT_FOUND` → рядок `device_tokens` видаляється; інші токени того ж юзера доставляються (fan-out незалежний per-token).
- **AC-4:** Retryable (5xx/429/мережа) → `push_attempts++`, `push_next_attempt_at=now()+backoff(attempts)`, `pushed_at` лишається NULL; після `MAX_ATTEMPTS` → `push_failed_at=now()` (dead-letter, не тягнеться в pending).
- **AC-5:** `pending()`-селект тягне лише `notifications` де `pushed_at IS NULL AND push_failed_at IS NULL AND (push_next_attempt_at IS NULL OR push_next_attempt_at<=now())`.
- **AC-6:** `EmailChannel::from_env()` резолвить `Noop`/`Log`/`SendGrid` за `EMAIL_PROVIDER`; unset → `LogEmail` (детермінований, CI-safe, без мережі/грошей).
- **AC-7:** email_worker для кожного `PendingEmail` викликає активний `EmailChannel.send()`; 2xx → watermark; помилка → `email_attempts++`/`next_attempt_at`/`last_error`, watermark НЕ ставиться.
- **AC-8:** `SendGridEmail` робить `POST api.sendgrid.com/v3/mail/send` (Bearer `SENDGRID_API_KEY`, from `EMAIL_FROM`, коректні to/subject/text+html) з `build_confirmation` та нових `build_ticket`/`build_receipt`/`build_refund`/`build_guest_magic_code`.
- **AC-9:** Типи `ticket`,`receipt`,`refund`,`guest_magic_code` ставляться в `email_outbox` відповідними доменними подіями (mint, refund, guest checkout/claim) і доставляються тим самим worker+seam.
- **AC-10:** `GET /api/admin/channels/health` повертає точні 24г-лічильники `sent/failed/pending/dead_lettered` + `provider` + `configured` для обох каналів; неадмін → 403.
- **AC-11:** `POST /api/admin/channels/test {channel,target}` синхронно шле тест через активний provider і повертає `delivered/provider_ref/error` БЕЗ запису в outbox.
- **AC-12:** Коли канал не сконфігурований (Noop/Log) — жоден шлях не 500-ить через відсутні creds (graceful degrade, як payment `NoopProvider`); health показує `configured=false`.
- **AC-13:** `.env.example` + `configuration-manual.md` документують `FCM_PROJECT_ID`,`FCM_SA_KEY`,`EMAIL_PROVIDER`,`SENDGRID_API_KEY`,`EMAIL_FROM`; `docs/spec.md` оновлено ПЕРЕД кодом.
- **AC-14:** guest magic-code email доставляється неавторизованому отримувачу; claim-лінк веде на claim без існуючого акаунта — розблоковує guest→registered activation KPI.

## Security Acceptance Criteria
- **SEC-1 (secret-leak):** `FCM_SA_KEY` та `SENDGRID_API_KEY` — тільки з env/секрет-стору, НІКОЛИ не логуються, не потрапляють у health/test-response, не хардкодяться (rules §10); SA-key читається з path, не інлайниться.
- **SEC-2 (token-leak):** OAuth2 access-token не логується й не повертається в API; кеш у пам'яті процесу, не в БД.
- **SEC-3 (authz/IDOR + relay):** `/api/admin/channels/*` під admin-guard (як інші `/api/admin/*`); неавторизований/неадмін → 403; test-send rate-limited, щоб не стати open email/push-relay.
- **SEC-4 (token-replay/rate-limit):** `guest_magic_code`-токен у email — single-use, short-TTL, rate-limited per email/device; лист не розкриває чужих PII; посилання підписане (signed-token seam).
- **SEC-5 (opt-out):** email до `users.blocked=true` та opted-out НЕ шлеться (наявний `WHERE u.blocked=false` зберегти; поважати per-channel mute).
- **SEC-6 (injection):** HTML-email екранує user-controlled поля (`event_title`, ім'я дарувальника, `message`) — no HTML-injection у листі.
- **SEC-7 (leak у payload):** FCM data-payload не містить секретів/сирих QR-токенів; push data мінімальний (deep-link id, не сам квиток).
- **SEC-8 (error-leak):** dead-letter/`last_error` зберігає message провайдера без credentials; помилки не відкривають внутрішню інфру клієнту.
- **SEC-9 (SSRF/oversized):** token-endpoint і FCM/SendGrid-хости — фіксований allowlist URL, без user-controlled хоста; `reqwest` без redirect-follow на довільні хости; test-send target валідований (не внутрішній host).

## Failure Modes / Error Mapping
| Умова | Обробка | Наслідок |
|---|---|---|
| FCM 2xx | `pushed_at=now()` | delivered, attempts не рушить |
| FCM `UNREGISTERED`/`NOT_FOUND` (Permanent) | `DELETE device_tokens` | інші токени юзера доставлені |
| FCM 5xx/429/мережа (Retryable) | `attempts++`, `next_attempt_at=now()+backoff` | pending, `pushed_at` NULL |
| FCM attempts ≥ MAX | `push_failed_at=now()` | dead-letter, drain пропускає |
| OAuth2 token expired (`>exp-60s`) | re-mint | новий Bearer |
| SA-key/creds відсутні | graceful degrade (Noop/Log) | продукт робочий, `configured=false` |
| SendGrid 202 | watermark `sent_at` | delivered |
| SendGrid 401/5xx | `email_attempts++`, `last_error`, next tick retry | pending |
| admin/channels/* без токена/неадмін | 403 | authz |
| test-send понад rate-limit | 429 | anti-relay |
| test-send target = внутрішній host | reject (allowlist) | SSRF-guard |
| guest magic-code повторно/expired | 410 consumed | anti-replay |

## Test Matrix
> Surface = API. Клієнтських (UI) поверхонь немає (Android/Web attendee не змінюється, admin health-UI — окрема задача), тому Base UI / UI / Playwright-стадії — N/A (⬜).

| AC / SEC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 FCM 2xx→pushed_at | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-2 OAuth2 token cache (2 send→1 mint) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-3 UNREGISTERED→drop token | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-4 retry/backoff→dead-letter | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-5 pending()-предикат | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-6 from_env resolves impl (default=Log) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-7 email_worker→seam+watermark | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-8 SendGrid POST+templates | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ | ✅ |
| AC-9 transactional-типи в outbox | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-10 health 24г-лічильники / 403 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-11 test-send sync, no outbox | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-12 graceful degrade (no creds) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-13 env+docs+spec | ⬜ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-14 guest magic-code delivered | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-1 keys not in logs/health/test (grep-assert) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-2 access-token not logged/returned | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-3 admin-guard 403 + test rate-limit 429 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-4 magic-code single-use/TTL/cap→410/429 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-5 blocked/opted-out not sent | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-6 HTML-escape (`<script>`) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-7 no raw qr_token in FCM payload | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-8 last_error без credentials | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-9 SSRF allowlist (internal host rejected) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |

**Стадійні нотатки:** Container-first, `docker-compose` one-command, порти через `.env` (rules §8). Мокнутий token-endpoint/FCM/SendGrid-хост у Unit+Integration (лічильник викликів для token-cache; captured-output grep для SEC-1/SEC-2). Кожна сюїта — positive + negative + `[SEC]`-вектори; 100% unit+integration coverage (rules §6/§7). Curl Prod/Prod Stack — лише на сконфігурованому staging-провайдері; на CI/dev — `LogEmail`+мок (реальних грошей/мережі немає, payment-last principle).
