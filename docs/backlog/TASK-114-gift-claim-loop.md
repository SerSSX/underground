# TASK-114 — Наскрізний frictionless gift/claim віральний контур
**Status:** IN PROGRESS (backend + web claim-landing DONE, verified live)
**Created:** 2026-06-29  **Updated:** 2026-07-02

> **Реалізовано:** увесь backend контуру (TASK-110 gift/claim, TASK-115 passwordless, TASK-113 anti-abuse,
> `GET /api/gifts/:token` публічне прев'ю — 100% coverage) + **web claim-landing** `web/app/g/[token]/page.tsx`
> (browse-before-auth: показ подарунка без логіну → passwordless email-код → claim) + `web/lib/api.ts`
> (giftPreview/requestEmailCode/verifyEmailCode/claimTicket/giftTicket). **Verified live** через preview проти
> реального API: `npm run build` OK; сторінка рендерить справжні дані («Аня дарує тобі квиток», Kadaitcha·Live, Київ);
> UI-крок request-code просуває до поля коду; 0 console-помилок. verify+claim покриті cargo-тестами.
> **Web SEND-сторона DONE (verified live):** `web/app/me/tickets/page.tsx` — кнопка «Подарувати лінком» → `giftTicket`
> → одноразовий share-лінк `/g/<token>` + Copy/Telegram/WhatsApp; квиток стає `pending_claim` («⏳ Очікує отримання»).
> Повна web-петля SEND→RECEIVE перевірена: Bob подарував → згенеровано лінк → лінк відкрив лендінг «Bob дарує тобі квиток».
> `npm run build` ✓.
> **Android RECEIVE DONE (verified on real device — moto_g05):** `ui/gift/ClaimGiftScreen`+VM, deep-link
> `underground://g/<token>` (manifest intent-filter + `MainActivity` + `AuthViewModel.refresh()`), API у `EventApi`/`AuthApi`.
> Прогнано на телефоні через adb (build `-PapiBaseUrl` + `adb reverse tcp:8091`): deep-link → лендінг із живими даними →
> email→код (899433 з debug-логу) → verify створив акаунт → claim → **DB: newpal@demo.co володіє active-квитком
> Kadaitcha·Live**, застосунок перейшов у онбординг. Побіжно виправлено латентний баг: `UndergroundTheme` не мала
> кореневого `Surface` → bare `Text` рендерився чорним-по-чорному (невидимий wordmark/заголовки на auth/gift-екранах).
> `compileDebugKotlin` ✓. `build.gradle.kts` тепер приймає `-PapiBaseUrl` (дефолт незмінний).
> **Android SEND DONE (verified on real device — moto_g05):** `ETicketScreen` — кнопка «Подарувати лінком» → діалог
> email → `TicketsViewModel.giftTicket` (`POST /api/tickets/:id/gift`) → одноразовий лінк `${WEB_BASE_URL}/g/<token>` →
> системний share-sheet (`Intent.ACTION_SEND`: «Дарую тобі квиток: <link>» → Telegram/Gmail/Copy). Квиток → `pending_claim`,
> бейдж «⏳ Очікує отримання» (`StatusBadge`). Прогнано на телефоні: лінк `…/g/32fee012…` згенеровано, share-sheet відкрито,
> **DB: `ticket_gifts`(to_email=giftpal@demo.co, status=pending, TTL 48h) + ticket.status=pending_claim** для «Kadaitcha·Live».
> Повна віральна петля тепер крос-платформна (web SEND+RECEIVE, Android SEND+RECEIVE — усе перевірено на пристрої).
> **Adversarial review (4 виміри × 3 верифікатори):** 9 хибних тривог відсіяно, 1 CONFIRMED (3/3) — pre-existing §10-порушення
> `WEB_BASE_URL` захардкоджено на dev-LAN у `build.gradle.kts` (споживач gift-лінка `ETicketScreen`). ВИПРАВЛЕНО: тепер конфігуроване
> `-PwebBaseUrl=` з дефолтом `http://localhost:3001` (як `API_BASE_URL`); у `app/src` немає жодного захардкодженого IP. Build ✓.
>
> **Revoke gift DONE (закрито specced-but-unbuilt TASK-114-B `DELETE /tickets/:id/gift-link`; verified on device):**
> API `DELETE /api/tickets/:id/gift` → `tickets::revoke_gift` — owner-only, атомарний (lock-order gift→ticket як у claim, без deadlock),
> race-safe (якщо вже claimed — немає `pending` gift → Conflict, claim виграє BR-018), money-neutral (escrow недоторканий);
> gift→`revoked`, ticket→`active`. **100% coverage**, +3 тести (`revoke_reclaims_pending_gift` з [SEC] token-dead, `revoke_requires_owner` [SEC],
> `revoke_without_pending_gift_conflicts`); fmt/clippy(-D) зелені. FE: `EventApi.revokeGift`, `TicketsViewModel.revokeGift`,
> `ETicketScreen` кнопка «Скасувати подарунок» на `pending_claim`. **Прогнано на пристрої:** ctrl мав pending_claim «Free Event» →
> «Скасувати подарунок» → DELETE 200 → список показав «✓ Активний»; **DB: ticket=active, gift=revoked**.
> **Web-паритет (verified у browser-preview):** `web/lib/api.ts revokeGift` + `web/app/me/tickets` кнопка «Скасувати подарунок»
> на `pending_claim` (confirm-modal). Перевірено вживу: клік → confirm → нотіс «Подарунок скасовано, квиток повернувся» →
> квиток «✓ Активний», QR повернувся, 0 console-помилок; **DB: ticket=active, gift=revoked**. Revoke тепер крос-платформний (API+Android+Web).
> Побіжно розблоковано Android unit-suite (зелена, 10 тестів): виправлено pre-existing `EventDtoTest` (позиційний
> конструктор `EventDto` зламався після додавання поля `organizer_id` → named-args + хелпер) і stale `FormatTest`
> (assert-и очікували стару поведінку — ISO-дата/код валюти; shipped-код давно дає «2 лип»/символ «₴»). `testDebugUnitTest` ✓.
> **Залишок:** Google one-tap (потребує founder OAuth client-id); web `/login` passwordless (built; verify блокує harness).
> «Режим дверей» (door-scan delegation) — DONE у TASK-118 (Android FE перевірено на пристрої).

## Raw Request
> «Головна віральна петля end-to-end на єдиному signed-token seam: створити подарунок-лінк → landing без акаунта (browse-before-auth) → забрати → акаунт in-place → re-share. Android + Web + API. Ініціатива INIT-GIFTLINK, Phase 1. Часткова реалізація = 0 K-factor — робимо контур цілісно, комісія НЕ з подарованих/free квитків.»

## Refined Description
**Scope:** Замкнути наскрізну віральну петлю на ОДНОМУ HMAC-signed opaque token seam (`kind ∈ {gift, view}`), для free та вже-куплених paid квитків, на трьох поверхнях:
- **API (Rust/Axum):** новий `api/src/gift.rs` (mint gift-link, публічний OG-payload, атомарний single-use claim, revoke, share-link `kind=view`); passwordless email-код (`/api/auth/email/start` + `/verify`) як виняток inline-auth; таблиці `gift_tokens`, `email_login_codes`; розширення `tickets.status` значенням `unclaimed` (постачається залежним TASK-111); telemetry воронки через наявний `telemetry.rs` seam.
- **Web (Next.js public):** SSR-роути `/g/[token]` (gift landing з OG/Twitter meta, цінність ДО логіну, розмитий QR, inline Google one-tap + email-код) і `/t/[token]` (guest ticket view, `kind=view`); claimed-success стан з ОБОВʼЯЗКОВИМ вихідним share/gift-CTA.
- **Android (Kotlin/Compose):** пункт «Подарувати» на active-квитку → `gift-link` → системний share-sheet; deeplink `underground://gift/:token` + https AppLinks на `/g`,`/t` → landing-екран (цінність + Google one-tap / email-код) → success-екран з тим самим re-share CTA.

**Non-goals:** реальний PSP / live-платіж / зміна формул комісії у `escrow.rs` (seam лишається `NoopProvider`, комісія з gift/free = 0 за визначенням); paid GUEST checkout за гроші анонімом (окрема ініціатива — тут paid-gift дарує вже КУПЛЕНИЙ донором квиток); Wallet/print/ICS; повна Афіша-ранжування/DM-композер/followers-екрани; staff door-scan; rotating/HOTP QR (QR лишається наявний статичний `qr_token`, прихований до claim); SMS/телефон-верифікація.

**Impacted UCs:** UC-005 (розширюється gift-link-варіантом), UC-001 (додається passwordless email-код як виняток inline-auth); **new UC required — «UC-Gift-Claim: наскрізний gift/claim без акаунта» (див. TASK-101 spec reconciliation)** — фіксується у `docs/spec.md` ПЕРЕД кодом: стан `unclaimed`, lifecycle signed gift-token, інваріант «no-commission-on-gift/guest/free», passwordless-виняток.

**Impacted BR/WF:** BR-003 (уточнення: комісія НЕ нараховується на gift/guest/free — потребує spec-редагування у TASK-101); BR-006 (escrow-інваріант зберігається: жоден gift/claim/revoke endpoint не викликає `escrow.rs`/провайдера); BR-010 (дії над квитком — лише власник у валідному статусі; поширюється на gift-link/revoke); BR-007/BR-017 (identity: passwordless email-акаунт лінкується за email, не дублює Google-акаунт). WF-Escrow: donor-ескроу лишається на `donor_id` до claim/expiry (даруємо ENTRY, право disputes у донора, як у наявному `transfer` BACK-1).

**Dependencies:**
- **TASK-101** — spec reconciliation (product-first, new UC, no-commission-on-gift, passwordless-виняток) — governing §2, ПЕРЕД кодом.
- **TASK-111** (INIT-TICKET-UNCLAIMED) — стан `tickets.status='unclaimed'` + міграція; технічний блокер, розблоковує claim seam.
- **TASK-112** (INIT-DEEPLINKS) — Android AppLinks/`assetlinks.json` + OG-image route для `/g`,`/t`; без нього K-factor share-петлі ≈ 0.
- **TASK-113** (INIT-CHANNELS) — робочі email+FCM канали для доставки коду/подарунка/re-share (email-код через `email_outbox` seam).
- **TASK-115** (INIT-ANTIABUSE) — rate-limit/device/email-cap seam; SEC-4 — УМОВА готовності, не окрема задача.
- **TASK-110** (INIT-BROWSE-AUTH) — browse-before-auth + persist anon-intent → merge при inline-login.
- Наявні seam-и: `tickets.rs` (mint/fetch/transfer BACK-1 policy), `telemetry.rs` (`log`), `auth.rs` (issue JWT/Google verify), `email_outbox` (0026), `config.rs` (нові env — потребує апруву §10).

**Risks / Open Questions:**
- Нові env (`SHARE_TOKEN_SECRET`, `PUBLIC_WEB_ORIGIN`, `DEEPLINK_HOST`, `GIFT_TOKEN_TTL`, `EMAIL_CODE_TTL`, `SHARE_LINK_RATE_LIMIT`) — вимагають founder-апруву (§10 forbidden без апруву).
- Публічний неавторизований web-layout: наразі існує `web/app` — переконатись, що public gift-роути не тягнуть auth-guard (перетин з TASK-016).
- Android release keystore SHA-256 fingerprint для `assetlinks.json` (dev+prod) — постачається TASK-112.
- Identity-merge guest email-акаунт → Google з тим самим email: злиття, не дублювання (BR-007/017) — узгодити у TASK-110.
- `passwordless` як виняток до Google-only — потребує явного founder-апруву + spec (TASK-101).

## Estimation
**Level:** HIGH (декомпозовано)  **Justification:** три клієнтські поверхні (API+Web+Android), новий крос-cutting security-seam (signed single-use token + passwordless auth), три міграції, 5 зовнішніх залежностей, повний воронковий telemetry-контур і адверсарний SEC-набір. Перевищує LOW-межу за §12.5 → MUST decompose у LOW/VERY-LOW-листя по шарах (schema→API→Web→Android→telemetry) до коду.

## Sub-tasks (DECOMPOSED)
| Sub-task ID | Title | Estimation | Status | Depends On |
|---|---|---|---|---|
| TASK-114-A | DB-схема: `gift_tokens`, `email_login_codes`, `tickets.donor_id`, partial-unique + free-index guard (міграції 0033–0035) | LOW | READY | TASK-101, TASK-111 |
| TASK-114-B | API — signed gift-token seam: `POST /tickets/:id/gift-link`, `DELETE /tickets/:id/gift-link`, `POST /tickets/:id/share-link` (mint/revoke, HMAC, partial-unique, escrow-інваріант) | LOW | READY | TASK-114-A, TASK-115 |
| TASK-114-C | API — публічний `GET /api/gift/:token` (no-auth OG-payload, без `qr_token`, enumeration-safe) | VERY-LOW | READY | TASK-114-A |
| TASK-114-D | API — атомарний `POST /api/gift/:token/claim` (FOR UPDATE single-use, age-gate, free-dedup, no-commission) | LOW | READY | TASK-114-A, TASK-114-C |
| TASK-114-E | API — passwordless email-код: `POST /api/auth/email/start` + `/verify` (hash-код, TTL, attempt-cap, no-enumeration, JWT) | LOW | READY | TASK-114-A, TASK-113 |
| TASK-114-F | API — telemetry воронки: `link_created→link_opened→value_viewed→claim_started→claim_succeeded→registered→re_shared` на наявному seam (token-nonce, не raw) | VERY-LOW | READY | TASK-114-B, TASK-114-D |
| TASK-114-G | Web — SSR `/g/[token]` gift landing (OG/Twitter meta, цінність ДО логіну, inline Google one-tap + email-код, стани claimable/expired/claimed/revoked) | LOW | READY | TASK-114-C, TASK-114-D, TASK-114-E, TASK-112 |
| TASK-114-H | Web — claimed success + ОБОВʼЯЗКОВИЙ re-share CTA + `/t/[token]` guest ticket view (`kind=view`) | LOW | READY | TASK-114-G, TASK-114-B |
| TASK-114-I | Android — «Подарувати» на MyTickets → gift-link → системний share-sheet; бейдж `unclaimed` + «Скасувати подарунок» | LOW | READY | TASK-114-B, TASK-112 |
| TASK-114-J | Android — claim через deeplink/AppLinks: landing-екран (цінність + Google one-tap/email-код) + success-екран з re-share CTA; email-код flow у AuthScreen | LOW | READY | TASK-114-D, TASK-114-E, TASK-112 |

## Execution Order
1. TASK-101 (spec: new UC + no-commission + passwordless-виняток) — governing, ПЕРЕД усім.
2. TASK-114-A (schema) → TASK-114-B, TASK-114-C.
3. TASK-114-D (claim) → TASK-114-E (passwordless) паралельно з D після A.
4. TASK-114-F (telemetry) після B+D.
5. TASK-114-G → TASK-114-H (Web) паралельно з TASK-114-I → TASK-114-J (Android) після API-листя (B–E) готові.
6. Фіча вмикається лише коли TASK-115 anti-abuse контур зелений (умова готовності).

## Speck Reference
docs/specks/SDD-114-gift-claim-loop.md
