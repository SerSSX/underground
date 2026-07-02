# SDD-112 — Universal links + OG-превʼю для всіх share-обʼєктів

**Impacted UCs:** UC-002 (перегляд події — адитивна публічна OG-модель), UC-005 (передача квитка — gift-link seam), **new UC required (UC-012 «Share/deep-link розповсюдження й guest-landing», реєстрація у spec — залежність TASK-101)**
**Impacted BR/WF:** BR-006 (гроші лише через `escrow.rs` — інваріант: share-endpoints не рухають ескроу), BR-010 (дії над квитком лише власником у валідному статусі — reuse у gift-link), WF-Escrow (не змінюється; frozen-перевірка успадкована з `transfer_ticket`), **new BR required** (share-token single-use для gift, HMAC-signed, short-TTL — фіксація у spec перед кодом)

## Scope / Non-goals
**Scope:** Верифіковані Android App Links (`assetlinks.json` + `autoVerify`) + iOS AASA-заготовка; server-rendered web-роути `/events/[id]`, `/g/[token]`, `/i/[token]` з OG/Twitter-мета, JSON-LD Event та динамічним `opengraph-image` (1200×630); єдиний HMAC-signed `share_tokens` seam (`gift`|`event_invite`) з single-use/TTL/rate-limit; публічні no-auth read-моделі подарунка/події для landing+OG; guest gift-claim до auth-wall (reuse `transfer_ticket` money-policy — лише ENTRY); телеметрія атрибуції `share_token`. Модулі: `api/src/share.rs` (новий), `api/src/tickets.rs`, `api/src/events.rs`, `api/src/telemetry.rs`, `api/src/config.rs`, `api/src/routes.rs`; `admin/app/(public)/**` (SSR + og-image + well-known); `android/app/src/main/AndroidManifest.xml` + `android/.../ui/deeplink/**`.

**Non-goals:** iOS Swift-код; Wallet-паси; повний SEO-каталог (TASK-016); paid guest-checkout / gift-at-checkout грошовим PSP; ротація QR-токена; email-транспорт e-ticket (TASK-CHANNELS); внутрішній DM/repost; будь-яка мутація `escrow.rs`/`orders`/`payments`.

## Acceptance Criteria
- AC-1: Клік по https share-лінку події на пристрої з встановленою verified-апкою відкриває EventDetails без chooser-діалогу; без апки — відкриває web `/events/[id]`.
- AC-2: `/.well-known/assetlinks.json` віддається з `application/json`, містить `package_name` + `sha256_cert_fingerprints`; `adb pm verify-app-links` → статус `verified` для host.
- AC-3: Розшарений у Telegram/WhatsApp/Instagram/X лінк усіх трьох типів показує прев'ю з коректним title/description і 1200×630 зображенням (`og:image` + `twitter:card=summary_large_image`) + валідний JSON-LD `Event` на `/events/[id]`.
- AC-4: `opengraph-image` для gift містить рядок «‹Ім'я дарувальника› дарує тобі квиток» і назву події; для події без cover — брендований постер-фолбек без 500.
- AC-5: Анонім відкриває `/g/:token`, бачить подарунок і подію ДО будь-якого логіну; auth-wall лише на «Забрати».
- AC-6: Анонім тисне «Забрати» → Google Sign-in → claim успішний → квиток у «Мої квитки»; телеметрія фіксує `guest_signup_from_link` + `gift_link_claimed` зі звʼязком до `share_token`.
- AC-7: gift-token single-use — повторний claim → 409 `already_claimed`; після `expires_at` → 410; landing показує людський стан.
- AC-8: gift-link для квитка під спором (frozen escrow) → 409 (як `transfer_ticket`); вік-обмеження події застосовується до отримувача (claim невідповідного віку → 409).
- AC-9: Жоден share-endpoint (gift-link/claim/invite-link) не викликає `escrow.rs`/`payments`; gift передає лише ENTRY, order/escrow лишаються за початковим покупцем (знімок БД до/після ідентичний по `escrow_status`/`orders`).
- AC-10: `apple-app-site-association` віддається за `/.well-known/` з `application/json` і БЕЗ розширення `.json`.
- AC-11: Хост/origin/секрет беруться з env (`PUBLIC_WEB_ORIGIN`/`DEEPLINK_HOST`/`SHARE_TOKEN_SECRET`); жодного хардкоду домену/порту/секрету в коді (§10).
- AC-12: KPI-події (`share_link_created`/`share_link_visited`/`gift_link_claimed`/`guest_signup_from_link`/`re_share`) пишуться через `telemetry::log`; збій запису телеметрії НЕ ламає основну дію.
- AC-13: `GET /api/events/:id` розширено публічною OG-підмножиною (`cover_url`, `city`, `timezone`, `friends_going` avatars) адитивно — наявний контракт не змінено (не breaking, §10).

## Security Acceptance Criteria
- SEC-1: Token forgery — токен HMAC-підписаний (`SHARE_TOKEN_SECRET`); сервер верифікує підпис ПЕРЕД будь-яким читанням БД; невалідний підпис → 404 (не 401 — не розкривати існування).
- SEC-2: Guessing/enumeration — токен ≥128 біт ентропії; `GET /api/g` та `/api/i` повертають ІДЕНТИЧНУ 404 для неіснуючого й невалідного токена.
- SEC-3: Replay/double-claim — claim у транзакції з `FOR UPDATE` на `share_tokens`; `claimed_at` set атомарно; повторний → 409 (single-use для gift). Гонка 2 паралельних claim → рівно один 200.
- SEC-4: Rate-limit створення лінків — cap на `created_by` за вікно (`SHARE_LINK_RATE_LIMIT`) і email-cap на `recipient_email` → 429 (анти-спам інвайтами).
- SEC-5: Abuse guest-claim — device/email-cap на claim з одного джерела → 429 (проти фарму акаунтів масовими клеймами).
- SEC-6: OG-image SSRF/injection — `cover_url` лише з власного uploads-домену (allowlist); імена/повідомлення екрануються в `ImageResponse` (без HTML/SVG-інжекту в текст).
- SEC-7: PII exposure — `GET /api/g/:token` віддає лише ім'я+аватар дарувальника і публічні поля події; НЕ email отримувача, НЕ `qr_token`, НЕ order/escrow-дані.
- SEC-8: Open redirect — web-landing CTA веде лише на власний scheme / known internal path; `underground://` deeplink валідує шлях в allowlist перед навігацією.
- SEC-9: assetlinks/AASA не витікають зайвого — лише `package_name`+fingerprint (assetlinks) і appID+paths (AASA); жодних секретів.
- SEC-10: Age-gate bypass — claim застосовує `meets_age` до отримувача так само, як `transfer_ticket`, щоб gift не обходив віковий гейт.

## Failure Modes / Error Mapping
- Невідомий/невалідний/підроблений token (`GET /api/g|/i`, claim) → **404** (єдина відповідь, SEC-1/SEC-2).
- Token прострочений (`expires_at` < now) → **410 gone**; landing рендерить «Термін дії посилання минув».
- gift вже забрано (`claimed_at` set) → **409 already_claimed**; landing → «Подарунок уже забрано».
- gift-link не власником / квиток не `active` / frozen escrow → **409/403** (reuse `transfer_ticket`).
- claim отримувачем невідповідного віку → **409 age_restricted**.
- claim без auth → **401** `{signup_required:true, token}` (клієнт веде на Sign-in і повторює).
- Перевищення rate-limit / email-cap / claim-cap → **429**.
- OG-image для неіснуючого event/token або відсутнього cover → **безпечний брендований фолбек-плейсхолдер, НЕ 500**.
- `cover_url` поза uploads-allowlist → ігнорується → постер-фолбек (SEC-6).
- Збій `telemetry::log` → логується, основна дія повертає успіх (AC-12).

## Test Matrix
| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 | ✅ | ✅ | ⬜ | ✅ | ✅ | ⬜ | ✅ |
| AC-2 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-3 | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-4 | ✅ | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ |
| AC-5 | ⬜ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| AC-6 | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| AC-7 | ✅ | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ |
| AC-8 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-9 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-10 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-11 | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ | ✅ |
| AC-12 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-13 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-1 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-2 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-3 | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-4 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-5 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-6 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-7 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-8 | ✅ | ✅ | ⬜ | ✅ | ✅ | ⬜ | ✅ |
| SEC-9 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-10 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |

**Positive vectors:** assetlinks/AASA з коректним content-type; gift-link створення власником active-квитка → 200 з url+token; `GET /api/g/:token` no-auth → публічна модель; claim залогіненим → зміна власника + телеметрія; guest 401→signin→claim 200; OG-image 200/png для cover і фолбек; verified App Link відкриває апку.
**Negative vectors:** невідомий/невалідний token → 404 (однаково); expired → 410; already-claimed → 409; gift-link не-власником/не-active/frozen → 403/409; claim невідповідного віку → 409; OG для неіснуючого → фолбек не 500.
**[SEC] vectors:** підроблений HMAC → 404; enumeration → та сама 404; double-claim гонка → один 200/один 409 (`FOR UPDATE`); rate-limit/email-cap → 429; device/email claim-cap → 429; відповідь `/g` без email/qr_token/escrow; `cover_url` поза allowlist → фолбек (anti-SSRF); спецсимволи в name/message не інжектять SVG/HTML; знімок БД до/після — `escrow_status`/`orders` незмінні.
