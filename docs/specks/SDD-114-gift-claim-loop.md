# SDD-114 — Наскрізний frictionless gift/claim віральний контур (єдиний signed-token seam)
**Impacted UCs:** UC-005 (розширення), UC-001 (passwordless inline-auth), **new UC required — UC-Gift-Claim (див. TASK-101 spec reconciliation)**   **Impacted BR/WF:** BR-003 (no-commission-on-gift — spec-редагування у TASK-101), BR-006 (escrow-інваріант зберігається), BR-010, BR-007/BR-017 (identity-merge), WF-Escrow (donor-ескроу до claim/expiry)

## Scope / Non-goals
**Scope:** Наскрізна віральна петля «лінк без акаунта → цінність ДО логіну → забрати → акаунт in-place → re-share» на ОДНОМУ HMAC-SHA256 signed opaque token seam (`kind ∈ {gift, view}`, single-use для gift, short-TTL) для free та вже-куплених paid квитків. Поверхні: API (`api/src/gift.rs`, `auth.rs`, `tickets.rs`, `telemetry.rs`; міграції 0033–0035), Web (`web/app/g/[token]`, `web/app/t/[token]` — SSR+OG), Android (`android/.../ui` MyTickets gift-action, DeepLink landing+success). Provider-neutral: `escrow.rs` не чіпається; комісія з gift/free = 0.

**Non-goals:** реальний PSP/ledger/зміна формул `escrow.rs`; paid guest-checkout за гроші анонімом; Wallet/print/ICS; повна Афіша-ранжування/DM-композер/followers; staff door-scan; rotating/HOTP QR (лишається статичний `qr_token`, прихований до claim); SMS-верифікація (лише email-код + Google one-tap).

## Acceptance Criteria
- **AC-1:** Власник active-квитка викликає `POST /api/tickets/:id/gift-link` → отримує https-url з opaque signed token; квиток → `status='unclaimed'`, `donor_id=owner` (paid); partial-unique блокує другий активний gift-link на цей квиток.
- **AC-2:** `GET /api/gift/:token` без будь-якого `Authorization` повертає `{event, tier, donor_name, claimable}` і НЕ містить `qr_token`; `claimable=true` поки token не used/expired/revoked.
- **AC-3:** Анонім відкриває `/g/[token]` → у месенджері бачить OG-preview (`og:image=poster`, `og:description='<Donor> дарує тобі квиток'`) і повну картку цінності ДО логіну; auth-wall зʼявляється лише при натисканні «Забрати».
- **AC-4:** Перший авторизований claim (Google one-tap АБО email-код, на тій самій сторінці, без редіректу геть) переводить ticket `unclaimed→active`, `owner_id=claimer`, повертає `{token(JWT), user, ticket}`; token → `used` (single-use).
- **AC-5:** Другий claim того самого token → `409` already_claimed; expired → `410`; revoked → `404`. Гонка двох claim-ів дає РІВНО один active-квиток (`SELECT ... FOR UPDATE`).
- **AC-6:** Для paid-подарунка комісія НЕ нараховується; ескроу лишається на `donor_id` (право disputes у донора); одержувач отримує ENTRY-квиток без нового order/провайдера (`escrow.rs` не викликано).
- **AC-7:** Free-квиток дарується так само (`unclaimed→active` у одержувача) без виклику провайдера; повторний free-квиток на ту саму подію в одержувача → `409` з чистим повідомленням, token НЕ спалюється.
- **AC-8:** Екран успіху (Android + web) ЗАВЖДИ показує вихідний share/gift-CTA «Візьми друга — подаруй другий»; клік генерує новий gift-link (якщо є ще квитки) або share-лінк події; фіксується telemetry `re_shared`.
- **AC-9:** Власник може `DELETE /api/tickets/:id/gift-link` поки unclaimed → квиток → `active` до нього; після claimed → `409`.
- **AC-10:** Passwordless: `POST /api/auth/email/start {email}` мінтить 6-значний код (TTL 10 хв, доставка через `email_outbox`), `POST /api/auth/email/verify {email,code}` видає JWT і створює акаунт при першому вході (`password_hash=NULL`); >5 невдалих спроб блокує код.
- **AC-11:** Уся воронка інструментована на наявному `telemetry.rs` seam: `link_created, link_opened, value_viewed, claim_started, claim_succeeded, registered, re_shared` — кожна подія з `ticket_id`/token-nonce (НЕ raw token).
- **AC-12:** `POST /api/tickets/:id/share-link` мінтить `kind='view'` token → `/t/[token]`; `GET /api/gift/:token` для `kind='view'` показує деталі без claim-кнопки; QR лише авторизованому власнику.

## Security Acceptance Criteria
- **SEC-1 (token-integrity/leak):** Token — opaque, HMAC-SHA256 на `SHARE_TOKEN_SECRET`; у БД лише `token_hash` (як `password_resets`); raw token НІКОЛИ не логується/не потрапляє в `telemetry_events`; 128-bit nonce — не вгадуваний/не підроблюваний.
- **SEC-2 (single-use/replay):** `used_at` + `FOR UPDATE` гарантують рівно один claim; конкурентні claim / повтор запиту = безпечний no-op, без подвійного квитка чи подвійного списання місць.
- **SEC-3 (short-TTL):** `expires_at` (default `GIFT_TOKEN_TTL`=7 днів); протерміновані токени не claimable; лінива перевірка на кожному GET/claim.
- **SEC-4 (rate-limit/cap):** rate-limit на gift-link creation (per user), на claim та `email/start` (per IP+email+device), email-код `attempts ≤ 5`; захист від масового мінту / брутфорсу коду / enumeration (успадковано з TASK-115 seam). `>ліміт` → `429`.
- **SEC-5 (IDOR/authz):** gift-link/revoke/share-link — лише `owner` квитка (guard як `transfer_ticket`); чужий/невідомий квиток → `404`; claim власного подарунка (`claimer==donor`) → `409`.
- **SEC-6 (enumeration/leak):** `GET /api/gift/:token` не витікає `qr_token` у ЖОДНОМУ стані і не розрізняє «не існує» vs «протермінований» надлишковими деталями/timing; `donor_name` — лише display name, не email.
- **SEC-7 (age-gate/frozen escrow):** age-gate зберігається на claim (`meets_age` як у `transfer`) — не можна забрати квиток на 18+ подію не відповідаючи рейтингу; frozen(спір)-квиток НЕ можна подарувати; жоден gift/claim/revoke endpoint не викликає `escrow.rs`/провайдера і не змінює `orders.escrow_status` (BR-006).
- **SEC-8 (passwordless-injection/enumeration):** email-код — hash у БД, TTL, attempt-cap; `/start` не розкриває існування акаунта (однакова `204`); JWT видається лише після `verify`; узгоджено з `tokens_valid_after` watermark; email/code санітизуються (injection).
- **SEC-9 (deeplink-hijack):** AppLinks/deeplink — token валідується сервером, клієнт нічого не довіряє з URL; https-домен verified (`assetlinks.json`) щоб уникнути перехоплення gift-лінку іншим застосунком (постачається TASK-112).
- **SEC-10 (oversized):** тіла `claim`/`email/start`/`verify` обмежені body-size limit; надмірний payload → `413`, без паніки/leak.

## Failure Modes / Error Mapping
| Умова | Код | Поведінка |
|---|---|---|
| gift-link на used/returned/past квиток | 409 | без side-effect |
| gift-link на чужий/невідомий квиток | 404 | без розкриття існування |
| gift-link на квиток з активним gift-link | 409 | partial-unique conflict |
| gift-link на frozen(спір) квиток | 409 | escrow-інваріант (SEC-7) |
| GET/claim на невалідний/підроблений token | 404 | без сигналу про причину (SEC-6) |
| claim протермінованого token | 410 | expired, token не спалюється |
| claim вже claimed / revoked token | 409 / 404 | already_claimed / revoked |
| claim власного подарунка (claimer==donor) | 409 | SEC-5 |
| claim free-квитка у власника з наявним free | 409 | token НЕ спалюється (AC-7) |
| claim на age-restricted не за рейтингом | 409 | інлайн-підказка (SEC-7) |
| email/verify з невірним кодом | 401 | attempts++; >5 → код заблоковано |
| перевищено rate-limit | 429 | (SEC-4) |
| oversized body | 413 | (SEC-10) |
| DELETE gift-link після claimed | 409 | AC-9 |

## Test Matrix
| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 gift-link mint | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-2 public no-auth payload (no qr) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-3 OG value-before-auth | ⬜ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| AC-4 atomic claim + JWT | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-5 single-use / race | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-6 paid no-commission / escrow on donor | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-7 free dedup / token not burned | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-8 mandatory re-share CTA | ⬜ | ✅ | ⬜ | ✅ | ✅ | ⬜ | ✅ |
| AC-9 revoke while unclaimed | ✅ | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ |
| AC-10 passwordless start/verify | ✅ | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ |
| AC-11 funnel telemetry | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-12 share-link kind=view / t-page | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| SEC-1 token integrity / no raw leak | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-2 single-use replay | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-3 short-TTL expiry | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-4 rate-limit / cap → 429 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-5 IDOR / owner guard / self-claim | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-6 enumeration / no qr leak | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-7 age-gate / frozen / escrow untouched | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-8 passwordless no-enumeration | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-9 deeplink verified host | ⬜ | ✅ | ⬜ | ✅ | ✅ | ⬜ | ✅ |
| SEC-10 oversized body → 413 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
