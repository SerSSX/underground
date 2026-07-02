# TASK-110 — Стан квитка unclaimed + подарунок на незареєстрований email
**Status:** IN PROGRESS (API DONE, verified)
**Created:** 2026-06-29  **Updated:** 2026-07-02

> **Реалізовано (API-фундамент gift-контуру, verified `cargo test`):**
> - Міграція `0034_ticket_gifts.sql` (`ticket_gifts` + partial-unique «one pending per ticket»);
> - `api/src/tickets.rs`: `POST /api/tickets/:id/gift` (працює для БУДЬ-ЯКОГО email, навіть без акаунта — лочить квиток у
>   `pending_claim`, повертає single-use raw claim-token; лише hash зберігається) + `POST /api/tickets/claim` (bearer-token,
>   first-wins BR-018, age-gate на claim, self-heal на прострочення) + маршрути; TTL через `CLAIM_TOKEN_TTL_HOURS` (48h default, BR-022).
> - **Лагодить реальний баг:** наявний `transfer_ticket` падав на незареєстрованому email — новий gift-шлях це знімає.
> - 5 нових тестів (gift→claim без акаунта, lock+double-gift 409, [SEC] non-owner 404, invalid token 404, single-use) +
>   0 регресій у tickets_actions/reserve/validate.
> **Залишок (FE-листя, у складі TASK-114 gift-loop):** екран «Подарувати» (share-лінк) на Android/Web + claim-landing (з TASK-115 browse-before-auth).

## Raw Request
> TASK-110 «Стан квитка unclaimed + подарунок на незареєстрований email» (ініціатива INIT-TICKET-UNCLAIMED, Phase 1, орієнтовна оцінка MEDIUM). ФОКУС: tickets — стан unclaimed/pending_claim; можливість випустити/подарувати квиток на email без акаунта; закріплення за першим, хто claim. Технічний фундамент gift-контуру. Декомпозуй (schema, API, Android, Web).

## Refined Description
**Scope:** Зняти технічний блокер віральної gift-петлі. Ввести стан квитка `tickets.status='unclaimed'` (synonym у продукт-копірайті — «pending_claim») з одноразовим криптографічно-випадковим `claim_token` та TTL (`claim_expires_at`, дефолт +14 днів). `transfer_ticket` на невідомий email більше НЕ падає з `Validation` — переводить квиток у `unclaimed` і повертає `claim_url`. Публічний (анонім-дозволений) `GET /api/gift/:token` віддає превʼю ДО логіну (цінність-перед-авторизацією). `POST /api/gift/:token/claim` (auth) атомарно закріплює квиток за ПЕРШИМ авторизованим користувачем, який claim (unclaimed→active, owner_id=me). Дарувальник бачить статус «Надіслано / Переглянуто / Забрано», може нагадати (`gift-remind`, rate-limited) і відкликати (`gift-revoke`) поки не забрали. Кожен крок інструментований (`gift_sent→gift_viewed→gift_claimed→gift_signup`) для K-factor. Клієнти: Android (діалог «Подарувати», секція «Подаровані», deep-link `underground://gift/:token`) і Web (SSR-сторінка `/gift/[token]` з OG-тегами + секція «Подаровані» у `/me/tickets`).

**Non-goals:** Gift-at-checkout (`checkout{gift:{recipient_email}}`) — окремо. Guest checkout (оплата без акаунта) — окремо (тут gift лише над уже наявними free/куплен­ими квитками). Transfer по `@username`/контакту — окремо. Signed-token seam + OG-багата landing з inline-auth end-to-end, вихідний re-share-CTA — це наступна ініціатива INIT-GIFTLINK (SDD-018), НЕ тут. Фоновий sweep авто-повернення сидіння по TTL — резерв (тут expiry перевіряється лениво при preview/claim). Wallet/rotating-QR, реальний PSP/провайдерні зміни — гроші НЕ рухаються цією задачею.

**Impacted UCs:** UC-005 (Передача / продаж / повернення квитка — розширюється станом `unclaimed`); **new UC required** для gift-claim незалогіненим отримувачем (див. TASK-101 spec reconciliation — фіксується як новий стан + інваріанти у `docs/spec.md` ПЕРЕД кодом, rules §0/§2).

**Impacted BR/WF:** BR-010 (дії над квитком лише власником/у валідному статусі — додається `unclaimed` як проміжний стан, escrow-звʼязок покупця незмінний як у BACK-1). WF-Escrow — НЕ змінюється (гроші не рухаються; provider-neutral escrow seam `api/src/escrow.rs` недоторканий; frozen-перевірка успадковується з `transfer_ticket`).

**Dependencies:** INIT-ROADMAP-SPEC / TASK-101 — базовий spec-апдейт «product-first, payment last» + фіксування стану `unclaimed` та інваріантів ПЕРЕД кодом (governing, §0/§2). Наявні seam-и: `api/src/tickets.rs` (`transfer_ticket:255`, `meets_age`, frozen-перевірка), `api/src/telemetry.rs` (розширення name-переліку), `api/src/email.rs` / email-outbox (0026 — узагальнити на довільний recipient email АБО клієнтський share-URL шлях), `api/src/escrow.rs` (недоторканий). Google Sign-In робочий (TASK-002) — для frictionless claim; до готовності — claim через наявний auth-шлях. Міграції `0021_free_ticket_dedup` / `0030_orders_ticket_unique` — предикати оновити щоб `unclaimed` не спрацьовував хибно.

**Risks / Open Questions:**
- owner_id для unclaimed: рішення ПРОСТЕ — `owner_id=gifted_by` поки unclaimed, доступ до e-ticket заблоковано доки `status='unclaimed'` (без sentinel-user).
- Free-ticket unique index (0021): unclaimed квиток не має хибно блокувати claim, коли в отримувача вже є free-квиток на подію — перевірка дублю в claim-handler → 409, token НЕ спалюється.
- email-outbox зараз order-scoped (0026): узагальнення на довільний email vs повернення share-URL клієнту — обрати мінімальний шлях (за замовчуванням share-URL, email-remind опційно).
- Rate-limit/anti-abuse (SEC) — умова готовності, не окрема задача; повний signed-token/OG/deeplink контур — у INIT-GIFTLINK.

## Estimation
**Level:** MEDIUM → **декомпозовано** (rules §12.5/§12.6). MEDIUM тут перетинає ЧОТИРИ distinct layers (Infra/DB → API → Android → Web) + sequential phase-boundary (schema→API→UI) + distinct UC (новий gift-claim-UC поверх UC-005). За §12.6 такі межі MUST бути розбиті у LOW/VERY-LOW-листя перед кодом; жоден лист не перевищує LOW.
**Justification:** Одна міграція, ~5 endpoint-ів (transfer-зміна + 4 нових), два клієнти. Кожен окремий шар — LOW. Ескроу не чіпається (гроші не рухаються) → без HIGH-ризику. Signed-token/OG-landing винесені у INIT-GIFTLINK, що тримає цю задачу у MEDIUM-обсязі.

## Sub-tasks (DECOMPOSED)
| Sub-task ID | Title | Estimation | Status | Depends On |
|---|---|---|---|---|
| TASK-110-A | Spec reconciliation: стан `unclaimed` + інваріанти у `docs/spec.md` (UC-005 + new gift-claim UC) | VERY-LOW | READY | TASK-101 |
| TASK-110-B | Міграція `0033_ticket_claim.sql`: `unclaimed`-status, `claim_*` колонки, індекси, оновлення 0021/0030-предикатів | LOW | READY | TASK-110-A |
| TASK-110-C | API: `transfer_ticket` → `unclaimed` на невідомий email + `claim_url`; регресія existing-user | LOW | READY | TASK-110-B |
| TASK-110-D | API `gift.rs`: `GET /api/gift/:token` (анонім-превʼю + `claim_viewed_at`), `POST /api/gift/:token/claim` (атомарний, age-gate) | LOW | READY | TASK-110-B, TASK-110-C |
| TASK-110-E | API: `gift-remind` (rate-limited) + `gift-revoke` (owner-only) + `GET /api/me/tickets` gift-статус DTO + telemetry-name-и | LOW | READY | TASK-110-D |
| TASK-110-F | Android: діалог «Подарувати» (email/share) + секція «Подаровані» + deep-link `underground://gift/:token` → claim | LOW | READY | TASK-110-E |
| TASK-110-G | Web: SSR `/gift/[token]` (OG-теги, превʼю ДО логіну, inline-claim) + секція «Подаровані» у `/me/tickets` | LOW | READY | TASK-110-E |

## Execution Order
TASK-110-A (spec-first, §0/§2) → TASK-110-B (schema) → TASK-110-C (transfer-зміна) → TASK-110-D (gift preview+claim core) → TASK-110-E (remind/revoke/DTO/telemetry) → TASK-110-F ∥ TASK-110-G (клієнти паралельно на стабільному API). Код не починається доки всі листя LOW/VERY-LOW і TASK-110-A змерджено (spec-first).

## Speck Reference
docs/specks/SDD-110-ticket-unclaimed-state.md
