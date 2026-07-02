# TASK-115 — Browse-before-auth + passwordless email-код

**Status:** IN PROGRESS (API DONE, verified)

> **Реалізовано (verified `cargo test`, 12 тестів зелені):** passwordless email-код —
> міграція `0035_login_codes.sql`; `api/src/emailcode.rs` (`POST /api/auth/email/request-code` — 204 anti-enumeration,
> soft-cap; `POST /api/auth/email/verify-code` — atomic single-use claim, створення акаунта на першому вході, merge за
> email BR-007); `users::{find_by_email, create_passwordless_user}` (NULL password); маршрути. Browse-before-auth —
> публічні GET (events/detail/social) уже є. 6 integration + 3 lib unit + 0 регресій (auth 6, profile 5).
> **Залишок (FE):** guest-стан + auth-prompt-на-дію на Android/Web (у складі TASK-114/116).

**Status(orig):** READY
**Created:** 2026-06-29  **Updated:** 2026-06-29

## Raw Request
> «Анонім бачить події/деталі/подарунок до логіну; auth-wall лише в момент дії (claim/buy/RSVP). Passwordless email-код + Google one-tap. Прибрати signup-wall перед цінністю, зробити claim/guest frictionless. Ініціатива INIT-BROWSE-AUTH, Phase 1. Залежить від auth-доктрини (TASK-101). Декомпозувати на guest API/session, passwordless, Android, Web.»

## Refined Description
**Scope:**
- Browse-before-auth: анонім стартує на Афіші (не Onboarding/Auth); Деталі + соц-блок «друзі йдуть» видимі без логіну; `GET /api/discover` вже віддає trending аноніму — розрив переважно навігаційний.
- Auth-on-action gate: `save`/`RSVP`/`buy`/`gift-claim` підіймають `AuthBottomSheet` («Continue with Google» + у claim/guest-контексті «Отримати код на email»), без повного редіректу на `/login`.
- Passwordless email-код як ДРУГИЙ метод входу ЛИШЕ у claim/guest-flow (Google-only лишається доктриною для повних акаунтів): `POST /api/auth/email-code/request` + `/verify`, 6-значний код, TTL ≤10 хв, single-use, hashed, rate-limited; видає той самий app-JWT, що й Google.
- `POST /api/auth/google` (id_token→app-JWT, upsert за email, заповнити `users.google_sub`) — передумова для merge за email.
- Guest claim без акаунта: signed claim_token → публічний read-only перегляд квитка → «Забрати» через email-код inline → guest-placeholder promote → білет прив'язується.
- Identity-merge: перший Google/email-код-логін з email == guest_email раніше отриманих unclaimed-квитків → авто-claim в одній транзакції.
- Anon-intent persist: клієнтський device-scoped `pending_intent {kind, event_id?, claim_token?, state?}` → `POST /api/me/merge-intents` дренує ідемпотентно при першому логіні.
- Tracking всієї воронки (browse→gate→login→claim→re-share) через наявний telemetry seam.
- Provider-neutral: claim/guest FREE/RSVP не торкається грошей (NoopProvider); escrow seam недоторканий.

**Non-goals:**
- Email/password як загальний метод входу (TASK-AUTH / UC-001 — не розширюємо тут).
- SMS/phone auth, соц-логіни крім Google, refresh-token ротація.
- Rotating QR (TASK-QR), ticket tiers (TASK-TIERS), реальний PSP (Фаза 4) — лише інтеграційні точки.
- Повний онбординг-редизайн (TASK-ONB-1) — тут лише робимо його non-blocking.
- Gift-link creation / OG-landing / re-share-CTA (INIT-GIFTLINK / INIT-DEEPLINKS) — окремі задачі; TASK-115 споживає їхній claim_token seam.
- unclaimed-стан квитка сам по собі — приходить з INIT-TICKET-UNCLAIMED; тут споживаємо.

**Impacted UCs:** UC-002 (перегляд афіші — аноніму без wall), UC-003 (деталі гостю), UC-004 (guest reserve/checkout). Поведінка «passwordless email-код у claim/guest» + «browse-before-auth» — **new UC required** (розширення UC-001 «Вхід»): спершу spec-виняток у `docs/spec.md` з founder-approval, координується через TASK-101 (spec reconciliation) — див. Dependencies.

**Impacted BR/WF:**
- BR-007 (один google_sub = один USER; повторний логін не дублює) — підтверджується.
- BR-016/BR-017 (паролі/rate-limit) — email-код успадковує rate-limit-доктрину, але сам код НЕ пароль; окремий SEC-контур.
- WF-Auth — розширити гілкою `email-code (guest/claim)` поряд з Google (spec-зміна перед кодом).
- WF-Escrow — недоторканий (free/RSVP/claim на NoopProvider; гроші лише через escrow.rs).

**Dependencies:**
- **TASK-101** (auth-доктрина / INIT-ROADMAP-SPEC) — spec-виняток email-magic-code (лише claim/guest) має бути внесений і апрувнутий засновником у `docs/spec.md` ПЕРЕД кодом (rules §0/§2/§3). BLOCKING.
- **INIT-TICKET-UNCLAIMED** (unclaimed-стан + claim_token seam) — надає, що привласнювати; координувати міграцію `tickets`. Hard.
- **INIT-CHANNELS** (робочий email seam) — доставка коду; у dev код видимий у email-outbox (container-first).
- `0020_email_ci` (email CI unique) — вже є; потрібен для безпечного merge за email.
- Google Sign-In SDK (Android/Web) — наразі Toast-заглушка `AuthScreen.kt`; блокує клієнтську частину AC-3/AC-7.
- PaymentProvider seam (Noop/Test) — лише для guest PAID checkout; FREE/RSVP/claim незалежні.
- Telemetry seam (`api/src/telemetry.rs`) — вже є; лише додати події воронки.

**Risks / Open Questions:**
- **Founder-approval gate:** spec-виняток passwordless вимагає явного апруву — до нього код не стартує (AC-9). Ризик блокування Phase 1.
- User-enumeration: `request-code` мусить бути timing-invariant і завжди 204 (SEC-2).
- Gift-theft по відкритому першим лінку: mitigation — код на recipient_email (координація з INIT-ANTIABUSE magic-code; тут email-код виконує ownership-proof для claim).
- guest-placeholder модель (`users.claimed=false`) vs nullable `buyer_id` — обрано placeholder-варіант (простіший, без розповсюдження nullable по коду); підтвердити на schema-review.
- Merge-race: паралельний Google-логін + claim того ж email → потрібен `FOR UPDATE` (SEC-3/SEC-5).

## Estimation
**Level:** MEDIUM
**Justification:** Одна вертикальна фіча, але 4 розрізнені шари (guest API/session на Rust; passwordless-модуль+міграції; Android Compose; Web Next.js) + spec-gate + security-контур (hashed code, rate-limit, anti-enumeration, anti-double-claim, id_token-верифікація). Кожен шар окремо — LOW. Обсяг понад VERY-LOW, тому MEDIUM; за §12.5 MEDIUM = flag+confirm. За §12.6 код НЕ стартує, доки всі листя не LOW — тому нижче наведено обов'язкову планову декомпозицію на LOW-листя; кодування веде листя-таблиця, ця картка лишається координаційною (READY).

## Планова декомпозиція (LOW-листя — §12.6, код лише коли всі LOW)
Межі: distinct layer (API/Android/Web) + distinct concern (session/passwordless/guest-claim) + послідовність (spec→schema→API→UI). Усі листя оцінені LOW/VERY-LOW.

| Sub-task ID | Title | Layer | Estimation | Status | Depends On |
|---|---|---|---|---|---|
| TASK-115-A | Spec-виняток passwordless (email-код лише claim/guest) + non-blocking onboarding запис у `docs/spec.md` + founder-approval | Docs | VERY-LOW | READY | TASK-101 |
| TASK-115-B | Міграції: `0033_email_login_codes.sql` + `0034_guest_orders.sql` (`users.claimed`, `orders.guest_email`, `orders.claim_token`, `tickets.claimed_at`) | Infra/DB | LOW | READY | 115-A, INIT-TICKET-UNCLAIMED |
| TASK-115-C | API: passwordless — `POST /api/auth/email-code/request`+`/verify` (hash, TTL, single-use, attempts, rate-limit, always-204) | API | LOW | READY | 115-B, INIT-CHANNELS |
| TASK-115-D | API: `POST /api/auth/google` (JWKS verify, upsert за email, google_sub, promote guest-placeholder) | API | LOW | READY | 115-B |
| TASK-115-E | API: guest reserve/checkout `{guest:{email,name}}` + `GET /api/tickets/claim/:token` (public, masked) + `POST /api/tickets/claim/:token` (FOR UPDATE, single-use) | API | LOW | READY | 115-B |
| TASK-115-F | API: identity-merge (verified-email → claim unclaimed) + `POST /api/me/merge-intents` (idempotent, authz-safe) | API | LOW | READY | 115-D, 115-E |
| TASK-115-G | Android: AfishaScreen як start-destination аноніму + AuthBottomSheet + EmailCodeSheet + pending_intent (DataStore) + guest claim deep-link | Android | LOW | READY | 115-C, 115-D, 115-E, 115-F |
| TASK-115-H | Web: public Деталі/`/t/claim/[token]` без wall + AuthBottomSheet + EmailCodeSheet + pending_intent (localStorage) + intent-drain | Web | LOW | READY | 115-C, 115-D, 115-E, 115-F |
| TASK-115-I | Tracking: воронка browse→gate→login→claim→re-share через telemetry seam (обидва клієнти + server-side) | API/Clients | VERY-LOW | READY | 115-F |

## Execution Order
1. **TASK-115-A** (spec-виняток + founder-approval — BLOCKING gate).
2. **TASK-115-B** (schema first).
3. Паралельно API-шар: **115-C**, **115-D**, **115-E** → потім **115-F** (merge залежить від google+guest).
4. Клієнти паралельно після API-контракту: **115-G** (Android), **115-H** (Web).
5. **115-I** (tracking) — наскрізно, зашивається в кожен шар, фіналізується останнім.

## Speck Reference
docs/specks/SDD-115-browse-before-auth.md
