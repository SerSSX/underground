# SDD-115 — Browse-before-auth + passwordless email-код у claim/guest-контексті

**Impacted UCs:** UC-002, UC-003, UC-004 + **new UC required** (розширення UC-001 «Вхід»: browse-before-auth + passwordless email-код лише у claim/guest — spec-виняток вноситься через TASK-101 перед кодом)
**Impacted BR/WF:** BR-007 (google_sub унікальність), BR-016/BR-017 (rate-limit-доктрина, успадкована email-кодом; код ≠ пароль), WF-Auth (нова гілка `email-code (guest/claim)`), WF-Escrow (недоторканий — free/RSVP/claim на NoopProvider, гроші лише через `api/src/escrow.rs`)

## Scope / Non-goals
**Scope:**
- Browse-before-auth: анонім бачить Афішу+Деталі+«друзі йдуть» першим екраном; auth підіймається ЛИШЕ на дію (save/RSVP/buy/gift-claim) через bottom-sheet, не редірект.
- Passwordless email-код як ДРУГИЙ метод ЛИШЕ у purpose∈{login(guest-context), claim}; Google-only — доктрина для повних акаунтів.
- `POST /api/auth/google` (JWKS-верифікація id_token, upsert за email, `users.google_sub`).
- Guest reserve/checkout `{guest}` (free завжди; paid лише якщо provider дозволяє) → guest-placeholder (`users.claimed=false`) + `claim_token`.
- Публічний read-only перегляд подарованого квитка + inline claim через email-код.
- Identity-merge unclaimed-квитків за верифікованим email в одній транзакції.
- Anon-intent persist (клієнт) → `POST /api/me/merge-intents` idempotent drain.
- Воронкова telemetry через наявний seam.

**Non-goals:** email/password як загальний метод (TASK-AUTH); SMS/phone; refresh-ротація; rotating QR/tiers/реальний PSP; gift-link creation/OG-landing/re-share-CTA (INIT-GIFTLINK/DEEPLINKS — тут споживаємо claim_token seam); повний онбординг-редизайн (лише non-blocking).

## Acceptance Criteria
- **AC-1:** Анонім (без токена) відкриває застосунок → бачить Афішу з trending одразу (`android .../ui/afisha` start-destination; `web/app/(public)` SSR); жоден екран не редіректить на Onboarding/Auth до першої дії.
- **AC-2:** Анонім відкриває Деталі + соц-блок «друзі йдуть» без логіну; tap на save/RSVP/buy показує `AuthBottomSheet`, а не повний `/login`-редірект.
- **AC-3:** `POST /api/auth/google` з валідним id_token створює/знаходить user за email, заповнює `google_sub`, видає JWT; повторний логін тим же Google → той самий `user_id` (BR-007, без дубля).
- **AC-4:** `POST /api/auth/email-code/request {email,purpose}` → на email (dev: outbox) приходить 6-значний код; `/verify` з правильним кодом у межах TTL≤10хв видає валідний JWT+user; неправильний/прострочений → 401.
- **AC-5:** Guest без акаунта отримує безкоштовний квиток через `reserve{guest}` → повертається `claim_token`; `GET /api/tickets/claim/:token` працює БЕЗ auth (read-only public).
- **AC-6:** Guest «Забирає» квиток email-кодом (purpose=claim) → guest-placeholder promote / акаунт за email → білет прив'язується (`claimed=true`), `claim_token` → consumed; повторний claim → 409.
- **AC-7:** Identity-merge: user отримав guest-квиток на email X, потім уперше логіниться Google/email-кодом з тим самим X → усі unclaimed-квитки з `guest_email=X` авто-прив'язуються в одній транзакції.
- **AC-8:** `pending_intent`: анонім тапнув save → пройшов auth → save застосовано автоматично через `POST /api/me/merge-intents`, без повторного тапу; повторний дренаж того ж intent нічого не дублює (idempotent).
- **AC-9:** Spec-виняток email-magic-код (лише claim/guest, другий метод) присутній у `docs/spec.md` ДО мерджу коду, з позначкою founder-approval (координація через TASK-101).
- **AC-10:** Share-лінк події та gift-claim-лінк відкриваються на web та через Android deep-link (`underground://ticket/claim/:token`); для аноніма ведуть на публічний перегляд, не на login-wall.
- **AC-11:** Уся воронка (`event_view_anon`, `auth_gate_shown`, `login_success`, `guest_claim_started`, `guest_claim_completed`, `intent_merged`, `share_clicked`) пише telemetry через наявний seam.

## Security Acceptance Criteria
- **SEC-1 (leak/at-rest):** `email_login_codes` зберігає лише hash коду (SHA-256/argon2), НІКОЛИ plaintext; код одноразовий (`consumed_at`), TTL≤10хв, `attempts<5` інакше 429; сирий код/claim_token ніколи не логуються.
- **SEC-2 (enumeration/timing):** `request-code` rate-limited (≤5/email/год + per-IP cap) і завжди повертає 204 незалежно від існування email; відповідь timing-invariant — без user-enumeration через статус чи затримку.
- **SEC-3 (authz-IDOR/double-claim/replay):** `claim_token` signed single-use short-TTL; consume під `FOR UPDATE` у транзакції; повторний/паралельний claim → 409/404, білет не дублюється; consumed/expired token → 404/410 (replay заблоковано).
- **SEC-4 (token forgery):** Google id_token верифікується повністю (підпис через Google JWKS, `aud=GOOGLE_CLIENT_ID`, `iss∈{accounts.google.com, https://accounts.google.com}`, `exp`); підроблений / чужий-aud / прострочений → 401.
- **SEC-5 (authz-merge integrity):** identity-merge прив'язує лише квитки, чий `guest_email` СУВОРО == верифікований email логіну (CI через `0020_email_ci`); email verified ЛИШЕ через Google `email_verified=true` або успішний email-код того ж email — жодного merge за непідтвердженим вводом; merge під `FOR UPDATE`.
- **SEC-6 (privilege-escalation):** email-код дозволений ЛИШЕ для `purpose∈{login,claim}`; повний Google-акаунт може логінитись кодом лише на той самий email і НЕ ескалює прав; `purpose` поза множиною → 400/401.
- **SEC-7 (authz on intents):** `merge-intents` застосовує лише дії від імені caller; claim у intent валідує `claim_token` (не можна claim'нути чужий білет підставивши `event_id`); age-gate/published-перевірки reserve/checkout зберігаються для guest-шляху.
- **SEC-8 (injection/oversized):** усі body-поля (`email`, `code`, `name`, `intents[]`) валідуються за типом/довжиною/форматом; `email` нормалізується+обмежується; `intents[]` cap на розмір масиву; параметризовані SQLx-запити (без конкатенації) — захист від SQL/JSON-injection і oversized-payload.
- **SEC-9 (guest-placeholder):** guest-placeholder (`claimed=false`) не має пароля/сесії доти, доки не claimed; логіниться лише через verified email того ж рядка; `blocked`-перевірка застосовується до guest-шляху.
- **SEC-10 (PII leak):** публічний `GET /api/tickets/claim/:token` маскує recipient email; постер/подія публічні, owner-контакти — ні.

## Failure Modes / Error Mapping
| Умова | Код | Тіло/поведінка |
|---|---|---|
| `verify` невірний/прострочений код | 401 | `{error:"invalid_code"}`; `attempts++` |
| `verify` `attempts>=5` | 429 | `{error:"too_many_attempts"}`, вимога resend |
| `request-code` rate-limit (email/IP) | 204 | завжди 204 (SEC-2); внутрішньо дропається |
| `request-code` неіснуючий email | 204 | 204 (no enumeration) |
| Google id_token невалідний/чужий aud/exp | 401 | `{error:"invalid_id_token"}` |
| claim вже claimed / паралельний claim | 409 | `{error:"already_claimed"}` (FOR UPDATE loser) |
| claim_token невалідний/consumed/expired | 404 / 410 | public GET → 404; POST → 410 |
| guest paid checkout, provider не дозволяє | 401 | `{error:"auth_required", provider_action}` |
| reserve guest на past/age-gated/unpublished | 409 | гейт збережено (`{error:"gate_failed"}`) |
| merge-intent з чужим ticket_id без валідного token | 200 | intent у `skipped[]` (не застосовано) |
| `purpose` поза {login,claim} | 400 | `{error:"invalid_purpose"}` |
| oversized/malformed body | 400/413 | валідаційна помилка |

## Test Matrix
| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 (browse-before-auth) | ⬜ | ✅ `GET /api/discover` no-auth→200 trending | ✅ | ✅ start=Afisha empty TokenStore | ✅ Android+Web anon lands Afisha | ✅ | ✅ |
| AC-2 (auth-on-action gate) | ⬜ | ✅ RSVP/buy no-auth→gate contract | ⬜ | ✅ bottom-sheet renders | ✅ tap save→sheet, no redirect | ⬜ | ✅ |
| AC-3 (google upsert) | ✅ verifier mock | ✅ valid→200 google_sub; repeat→same id | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-4 (email-code happy) | ✅ gen/hash/expiry | ✅ request→verify→JWT | ✅ | ⬜ | ✅ EmailCodeSheet flow | ✅ | ✅ |
| AC-5 (guest reserve+public view) | ⬜ | ✅ reserve{guest}→token→public GET | ✅ | ⬜ | ✅ Web public claim page | ✅ | ✅ |
| AC-6 (email-code claim) | ⬜ | ✅ claim→owner set,claimed=true; repeat→409 | ✅ | ⬜ | ✅ «Забрати» flow | ✅ | ✅ |
| AC-7 (identity-merge) | ⬜ | ✅ guest(X)+google(X)→auto-claim in tx | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-8 (pending_intent idempotent) | ✅ merge dedup | ✅ merge-intents save; repeat no dup | ✅ | ✅ intent persists | ✅ auto-complete after login | ⬜ | ✅ |
| AC-9 (spec-виняток founder-approval) | ⬜ | ⬜ (docs-gate; grep `docs/spec.md`) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-10 (share/deep-link anon) | ⬜ | ✅ claim link no-auth→public | ⬜ | ✅ deep-link routes | ✅ Android deep-link + Web open | ⬜ | ✅ |
| AC-11 (funnel telemetry) | ✅ event-name schema | ✅ emits on each step | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-1 (hash/single-use/TTL) | ✅ hashing | ✅ no plaintext in DB; consumed→401 | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-2 (no enumeration/timing) | ⬜ | ✅ unknown email→204; >5/h→limited; timing-invariant | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-3 (anti-double-claim/replay) | ⬜ | ✅ 2 parallel claims→one 200 one 409; no dup | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-4 (id_token forgery) | ✅ JWKS/aud/iss/exp | ✅ bad aud/sig/exp→401 | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-5 (merge integrity) | ⬜ | ✅ guest_email≠login or unverified→no merge | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-6 (no priv-escalation) | ✅ purpose guard | ✅ google-acct code-login no extra rights; bad purpose→400 | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-7 (intent authz) | ⬜ | ✅ foreign ticket_id no token→skip | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-8 (injection/oversized) | ✅ validators | ✅ malformed/oversized→400/413; SQLx param | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-9 (guest-placeholder) | ⬜ | ✅ placeholder no session until claimed; blocked enforced | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-10 (PII mask) | ✅ mask util | ✅ public claim GET → no full recipient email | ✅ | ⬜ | ⬜ | ⬜ | ✅ |

**Модулі-прив'язки:** `api/src/auth.rs` (google + email-code + merge-intents), нові `api/src/claims.rs` (guest claim/public view), `api/src/orders.rs`/`api/src/tickets.rs` (guest reserve/checkout), `api/src/email.rs` (код через seam), `api/src/telemetry.rs` (funnel), міграції `api/migrations/0033_email_login_codes.sql`+`0034_guest_orders.sql`; `android/.../ui/afisha`, `.../ui/auth` (AuthBottomSheet/EmailCodeSheet), DataStore pending_intent; `web/app/(public)`, `web/app/t/claim/[token]`, `web/lib/api.ts`; `admin/*` — без змін. Provider-neutral: `api/src/escrow.rs` недоторканий (free/RSVP/claim на NoopProvider).
