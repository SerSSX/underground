# SDD-110 — Ticket unclaimed state + gift на незареєстрований email
**Impacted UCs:** UC-005 (розширення станом `unclaimed`) · **new gift-claim UC required** (див. TASK-101 spec reconciliation)   **Impacted BR/WF:** BR-010 (додано проміжний стан `unclaimed`, escrow-звʼязок покупця незмінний як BACK-1); WF-Escrow — НЕ змінюється (provider-neutral escrow seam `api/src/escrow.rs` недоторканий; гроші не рухаються)

## Scope / Non-goals
**Scope:** Стан `tickets.status='unclaimed'` (pending_claim) з одноразовим криптографічно-випадковим `claim_token` і TTL. `transfer_ticket` на невідомий email → `unclaimed` замість падіння. Публічний `GET /api/gift/:token` (превʼю ДО логіну). Атомарний `POST /api/gift/:token/claim` — закріплення за ПЕРШИМ авторизованим claim. `gift-remind`/`gift-revoke` для дарувальника. Gift-статус у `GET /api/me/tickets`. Клієнти: Android (діалог «Подарувати», секція «Подаровані», deep-link) + Web (SSR `/gift/[token]` + OG). Телеметрія `gift_sent→gift_viewed→gift_claimed→gift_signup→gift_reminded→gift_revoked`. Модулі: `api/src/tickets.rs`, новий `api/src/gift.rs`, `api/src/routes.rs`, `api/src/telemetry.rs`, `api/src/email.rs`, `api/migrations/0033_ticket_claim.sql`, `android/.../ui/tickets/**`, `android/.../ui/gift/**`, `web/app/gift/[token]/**`, `web/app/me/tickets/**`.
**Non-goals:** Gift-at-checkout; guest checkout; transfer по `@username`; повний signed-token/OG-landing/inline-auth/re-share end-to-end (→ INIT-GIFTLINK / SDD-018); фоновий TTL-sweep (тут lazy-expiry); Wallet/rotating-QR; реальний PSP (гроші не рухаються).

## Acceptance Criteria
- AC-1: `transfer_ticket` на email, ЩО Є в `users` → поведінка незмінна: `owner_id` негайно змінюється, `status='active'`, 200 (регресія, UC-005/BR-010).
- AC-2: `transfer_ticket` на email, ЩО НЕ Є в `users` → квиток переходить у `status='unclaimed'`, генерується `claim_token`, `claim_email` збережено, `claim_expires_at=now()+14d`, `gifted_by=me`; відповідь містить `claim_url`; логується `gift_sent`. НЕ повертає помилку.
- AC-3: `GET /api/gift/:token` для валідного невитребуваного токена (анонім, без `Authorization`) → 200 з превʼю (event title/starts_at/city/venue_address/poster_url/age_rating, tier_name?, gifted_by_name, gift_message?, status); при першому виклику ставиться `claim_viewed_at` і логується `gift_viewed`; повторний виклик НЕ перезаписує `claim_viewed_at`.
- AC-4: `POST /api/gift/:token/claim` залогіненим користувачем → `owner_id=me`, `status='active'`, `claimed_at` set, `claim_token` обнулено (NULL); логується `gift_claimed`; e-ticket (QR) стає доступним новому власнику.
- AC-5: Свіжозареєстрований користувач (signup під час claim) успішно забирає квиток тим самим потоком; логується `gift_signup` + `gift_claimed` (`has_account=false`).
- AC-6: Дарувальник у `GET /api/me/tickets` бачить `gift_status`, що коректно переходить `sent→viewed→claimed` відповідно до `claim_viewed_at`/`claimed_at`.
- AC-7: `gift-revoke` на `unclaimed` → квиток повертається дарувальнику (`status='active'`, очищення `claim_*`), `claim_token` недійсний; наступний `GET /api/gift/:token` → 410. `gift-revoke` на вже claimed → 409.
- AC-8: `gift-remind` повторно надсилає лінк на `claim_email` (email-seam); понад rate-limit → 429; на claimed квиток → 409.
- AC-9: Age-restricted подія: claim користувачем, що НЕ відповідає віку → 409 з інлайн-підказкою; `owner_id`/`status` не змінюються.
- AC-10: Термін лінка минув (`claim_expires_at < now()`) → `GET`/`claim` повертають 410; сидіння не звільняється автоматично (lazy, поза scope).
- AC-11: `docs/spec.md` оновлено новим станом `unclaimed` та інваріантами (сидіння НЕ звільняється поки не revoked/expired; escrow-звʼязок покупця незмінний) ПЕРЕД мерджем коду (§0/§2, TASK-110-A).
- AC-12: unclaimed-квиток НЕ зʼявляється як e-ticket отримувача до claim і НЕ підпадає під free-ticket unique index (0021) хибно; повторний claim тим самим `me` → ідемпотентно 200 з квитком.
- AC-13: Android deep-link `underground://gift/:token` відкриває превʼю-екран (працює для аноніма) → «Забрати» → Google Sign-In → claim → E-Ticket; секція «Подаровані» показує статус-бейдж і дії «Нагадати»/«Відкликати».
- AC-14: Web `/gift/[token]` — SSR з коректними OG-тегами (title/description/image), превʼю рендериться БЕЗ логіну; «Забрати квиток» → inline Google Sign-In → claim; стани expired/revoked/already-claimed мають окремий копірайт.

## Security Acceptance Criteria
- SEC-1: `claim_token` — криптографічно випадковий (UUIDv4 з префіксом `GIFT-`), `UNIQUE`, single-use (обнуляється у NULL при claim/revoke), short-TTL (`claim_expires_at`). Не вгадуваний, не перевикористовуваний.
- SEC-2: `GET /api/gift/:token` НЕ розкриває PII понад `gifted_by_name`+`gift_message`; НЕ повертає `qr_token`/scanning-секрет анонімному переглядачу (QR — лише після claim власнику).
- SEC-3 (authz/IDOR + race): claim атомарний — `UPDATE ... WHERE status='unclaimed' AND claim_token=$1 RETURNING` під `FOR UPDATE`; паралельні claim → лише один виграє, іншому 409; неможливо claim чужий квиток без валідного токена.
- SEC-4 (authz/IDOR): `gift-remind`/`gift-revoke` авторизовані лише для `gifted_by`; чужий `id`/токен → 403 (не 404-leak про існування).
- SEC-5 (rate-limit): rate-limit на створення gift на невідомий email та на `gift-remind` (email/device/IP cap); token-enumeration throttled → 429 при перевищенні.
- SEC-6 (age-gate bypass): age-gate `meets_age` виконується у момент claim (коли особа отримувача відома), НЕ пропускається; gift не обходить віковий бар.
- SEC-7 (escrow-інваріант): жоден gift/claim/remind/revoke endpoint не викликає `escrow.rs`/провайдера і не змінює `orders.escrow_status`; квиток під frozen/disputed order НЕ можна перевести в `unclaimed` (успадкувати frozen-перевірку з `transfer_ticket`).
- SEC-8 (token-replay/self-claim): single-use токен не приймається повторно після claim/revoke; self-claim дарувальником (`claimer==gifted_by`) заборонено/ідемпотентно без побічного минта чи маніпуляції лічильниками.
- SEC-9 (injection/oversized/leak): `gift_message ≤ 500` char (>500 → 400); email нормалізований (lowercase/trim); превʼю та email-шаблон HTML-escape (без XSS-інʼєкції в OG/лист); token не логується у відкритому вигляді.

## Failure Modes / Error Mapping
| Умова | HTTP | Тіло/нотатка |
|---|---|---|
| transfer собі (`recipient==me`) | 400 | `Validation` — незмінна валідація |
| `gift_message` > 500 char | 400 | `Validation` |
| claim/remind/revoke без auth (де потрібно) | 401 | `Unauthorized` |
| remind/revoke не-власником (`≠ gifted_by`) | 403 | `Forbidden` (без leak існування) |
| невідомий `token`/`ticket_id` | 404 | `NotFound` |
| claim age-restricted без відповідного віку | 409 | `Conflict` + інлайн-підказка |
| claim вже забраного квитка іншим | 409 | `Conflict` |
| remind/revoke на вже claimed | 409 | `Conflict` |
| transfer/gift квитка під frozen/disputed order | 409 | `Conflict` (escrow-інваріант, SEC-7) |
| повторний claim дублю free-квитка отримувача | 409 | `Conflict`, token НЕ спалюється |
| `claim_token` expired/revoked | 410 | `Gone` |
| rate-limit перевищено (remind/create) | 429 | `TooManyRequests` |
| escrow/провайдерний виклик з gift-шляху | — | заборонено кодом (SEC-7), не існує code-path |

## Test Matrix
| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 transfer existing-user регресія | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| AC-2 transfer unknown-email → unclaimed | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| AC-3 GET gift preview + claim_viewed_at (анонім) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| AC-4 claim → active/owner=me/QR | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| AC-5 signup-during-claim (has_account=false) | ✅ | ✅ | ✅ | ⬜ | ✅ | ⬜ | ⬜ |
| AC-6 donor gift_status transitions | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-7 gift-revoke → повернуто, токен 410 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| AC-8 gift-remind + rate-limit 429 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-9 age-gate at claim → 409 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| AC-10 expired token → 410 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| AC-11 spec updated ПЕРЕД кодом | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-12 unclaimed НЕ у e-ticket / НЕ ламає 0021; claim ідемпотентний | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-13 Android deep-link → claim → секція «Подаровані» | ⬜ | ⬜ | ⬜ | ✅ | ✅ | ⬜ | ⬜ |
| AC-14 Web SSR /gift + OG + inline-claim | ⬜ | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ |
| SEC-1 token random/unique/single-use/TTL | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| SEC-2 no qr_token/PII-leak анониму | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| SEC-3 race double-claim → один 409 | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-4 remind/revoke не-власником → 403 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| SEC-5 rate-limit/enumeration throttle → 429 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-6 age-gate bypass через gift заблоковано | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| SEC-7 escrow-інваріант (frozen → 409, нема escrow-call) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| SEC-8 token-replay/self-claim заблоковано | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-9 oversized/injection/escape/no-log | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |

> Container-first (§6/§7): 100% unit+integration coverage нового коду; кожна сюїта — positive + negative + [SEC] вектори; fail-fast стадії. UI-стадії (Base UI/UI) заповнені лише для клієнтських AC (Android/Web) — Playwright device-matrix для web `/gift/[token]`. Curl Prod — smoke на prod-стеку через `NoopProvider` (гроші не рухаються).
