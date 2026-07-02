# TASK-112 — Universal links + OG-превʼю для всіх share-обʼєктів

**Status:** DECOMPOSED
**Created:** 2026-06-29  **Updated:** 2026-06-29

## Raw Request
> Верифіковані universal/app links (Android assetlinks) + серверний OG/JSON-LD рендер сторінок події/подарунка/профілю для превʼю в Telegram/WhatsApp/Instagram. Атрибуція share_token. Декомпозуй (web OG, android links, share_token). Ініціатива INIT-DEEPLINKS, Phase 1, орієнтовна оцінка MEDIUM.

## Refined Description
**Scope:** Замкнути розповсюджувальну частину віральної петлі: кожне share-посилання (подія `/events/:id`, подарунок `/g/:token`, профіль/інвайт `/i/:token`) — це верифікований universal https-лінк, що (а) відкриває Android-апку якщо встановлена й верифікована, інакше — server-rendered web-landing з чітким CTA; (б) у месенджері показує брендоване OG/Twitter-прев'ю 1200×630 + JSON-LD; (в) несе `share_token` для атрибуції воронки share→visit→claim→signup→re-share. Єдиний HMAC-signed `share_tokens` seam (kinds `gift`|`event_invite`). Provider-neutral: жоден грошовий seam не чіпається — gift передає лише ENTRY (money-policy як `transfer_ticket`), `escrow.rs` недоторканий.

**Non-goals:** iOS Swift-код (лише заготовка AASA-файлу); Apple/Google Wallet-паси; повний публічний SEO-каталог (`/c`, `/city` — TASK-016/INIT-SEO); paid guest-checkout / gift-at-checkout грошовим провайдером; ротація/HOTP QR-токена; email-доставка e-ticket (канал — TASK-CHANNELS); внутрішній DM/repost share; повна frictionless guest-signup UX (це INIT-GIFTLINK/INIT-BROWSE-AUTH — тут лише транспорт + landing до auth-wall).

**Impacted UCs:** UC-002 (перегляд події — розширення публічної OG-моделі, адитивно), UC-005 (передача квитка — gift-link як share-seam поверх transfer money-policy). **new UC required** — «UC-012 Share/deep-link розповсюдження й guest-landing» (реєстрація нового UC + новий стан share-обʼєктів фіксується у spec ПЕРЕД кодом, залежність на TASK-101 spec reconciliation).

**Impacted BR/WF:** BR-006 (гроші лише через `escrow.rs`) — інваріант: жоден новий endpoint не рухає ескроу; BR-010 (дії над квитком лише власником у валідному статусі) — reuse у gift-link; WF-Escrow — не змінюється (успадкувати frozen-перевірку з `transfer_ticket`). **new BR required** — «BR-0NN: share-token single-use для gift, HMAC-signed, short-TTL» (у spec перед кодом).

**Dependencies:**
- TASK-101 — spec reconciliation (реєстрація UC-012 + нового BR share-token ПЕРЕД кодом, rules §2).
- INIT-ROADMAP-SPEC — product-first/payment-last контекст (gift лише ENTRY, NoopProvider).
- Публічний неавторизований web-layout: наразі існує ТІЛЬКИ `admin/`; потрібен public app-router layout (перетин з TASK-016) — вводиться під `admin/app/(public)/**` або окремий `web/`.
- Google Sign-in flow (SDD-002) — робочий inline signup для guest-claim; до готовності claim через наявний auth-шлях.
- Нові env (потребують апруву, rules §10): `PUBLIC_WEB_ORIGIN`, `DEEPLINK_HOST`, `SHARE_TOKEN_SECRET`, `SHARE_LINK_RATE_LIMIT`.
- Android release keystore SHA-256 fingerprint (dev+prod) для `assetlinks.json`.
- `transfer_ticket` money-policy (`api/src/tickets.rs`) — reuse як gift ENTRY-seam; `escrow.rs` НЕ чіпається.
- `telemetry::log` (`api/src/telemetry.rs`) — розширення name-переліку (дешеве).

**Risks / Open Questions:**
- App Links verification вимагає стабільного prod-хоста + фактичного signing-cert; dev-verification через `adb pm verify-app-links` до prod-домену.
- OG-image через Next `ImageResponse` — SSRF/injection ризик по `cover_url`/іменах (закрито SEC).
- Нові env потребують апруву власника (rules §10) — блокує старт кодових листків.
- Публічний web-layout — можливий scope-перетин з TASK-016; узгодити межу (тут лише 3 share-роути + landing).

## Estimation
**Level:** HIGH → **MUST decompose** (rules §12.5/§12.6). Орієнтовна MEDIUM недооцінює: задача перетинає 3 різні шари (API / Android / Web) + Infra (well-known статик + env), вводить нову таблицю й signed-token seam, 12 AC та 10 SEC-векторів. Межі декомпозиції: distinct layer (API/Android/Web/Infra) + sequential phase (schema → API → clients). **Justification:** жоден листок не перевищує один шар/один вузол відповідальності; кожен ≤ LOW і незалежно тестований.

## Sub-tasks (DECOMPOSED)
| Sub-task ID | Title | Estimation | Status | Depends On |
|---|---|---|---|---|
| TASK-112-A-schema | Міграція `00NN_share_tokens.sql` + індекси (rate-limit/email-cap) | VERY-LOW | READY | TASK-101 |
| TASK-112-B-token-seam | API: HMAC `share_tokens` seam (mint/verify, single-use, TTL) в `api/src/share.rs` | LOW | READY | TASK-112-A |
| TASK-112-C-gift-api | API: `POST /tickets/:id/gift-link`, `GET /api/g/:token`, `POST /api/g/:token/claim` (reuse transfer money-policy) | LOW | READY | TASK-112-B |
| TASK-112-D-invite-api | API: `POST /events/:id/invite-link`, `GET /api/i/:token`; розширити `GET /events/:id` публічною OG-моделлю (адитивно) | LOW | READY | TASK-112-B |
| TASK-112-E-telemetry | API: share-атрибуція — telemetry names + `share_token`/`referrer_id` пропси на існуючому seam | VERY-LOW | READY | TASK-112-B |
| TASK-112-F-wellknown | Infra/Web: `/.well-known/assetlinks.json` + `apple-app-site-association` (правильний content-type, без .json) з env-host | VERY-LOW | READY | — |
| TASK-112-G-web-ssr | Web: SSR-роути `/events/[id]`, `/g/[token]`, `/i/[token]` + `generateMetadata` (OG/Twitter/canonical) + JSON-LD | LOW | READY | TASK-112-C, TASK-112-D, TASK-112-F |
| TASK-112-H-og-image | Web: динамічний `opengraph-image` (Next `ImageResponse`, 1200×630) для 3 шляхів з poster-фолбеком | LOW | READY | TASK-112-G |
| TASK-112-I-android-links | Android: `autoVerify` intent-filter (https host з env) + `underground://` фолбек у AndroidManifest + `DeepLinkRouter` | LOW | READY | TASK-112-F |
| TASK-112-J-android-share | Android: системний share-sheet генерує universal https-URL; `GiftClaimScreen` для залогіненого | VERY-LOW | READY | TASK-112-I, TASK-112-C |

## Execution Order
1. TASK-101 spec reconciliation (UC-012 + BR share-token) — блокуючий.
2. TASK-112-A (schema) → TASK-112-B (token seam).
3. Паралельно на seam: TASK-112-C (gift-api), TASK-112-D (invite-api), TASK-112-E (telemetry).
4. TASK-112-F (well-known) — незалежний, паралельно з API.
5. TASK-112-G (web SSR) → TASK-112-H (og-image).
6. TASK-112-I (android links) → TASK-112-J (android share).

## Speck Reference
docs/specks/SDD-112-deeplinks-og.md
