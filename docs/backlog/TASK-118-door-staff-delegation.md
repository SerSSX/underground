# TASK-118 — Делегований door-scan + безпечний QR + вердикти

**Status:** IN PROGRESS (API DONE, verified)

> **Реалізовано (verified `cargo test`):** міграція `0037_event_staff.sql` (`event_staff` scoped scan-only +
> `staff_invites` single-use token); `api/src/staff.rs` — `POST /api/events/:id/staff/invite` (owner, single-use токен),
> `POST /api/staff/accept` (redeem → scoped `event_staff`), `POST /api/events/:id/scan` (owner АБО delegated staff;
> event-scoped вердикти valid/already_used/**wrong_event**/returned/inactive/not_found). Наявний owner-only
> `POST /api/tickets/validate` не зачеплено. 4 [SEC]-тести + 0 регресій; повний суіт 239 passed / 0 failed.
>
> **Android FE DONE (core TASK-118-E, verified on real device — moto_g05; backend НЕ змінювався):**
> - Organizer invite: `EditEventScreen` секція «Контроль на дверях» → `MyEventsViewModel.inviteStaff` (`POST /events/:id/staff/invite`)
>   → deep-link `underground://staff/<token>?e=<eventId>&t=<title>` через системний share-sheet.
> - Staff accept: manifest intent-filter `underground://staff`, `MainActivity.extractStaffInvite` + `Root` (accept-wall — вимагає логін),
>   `ui/staff/AcceptStaffScreen`+`AcceptStaffViewModel` (`POST /staff/accept`) → «✓ Ти контролер» + назва події + кнопки Сканувати/Готово.
> - Event-scoped scan: `ScanTickets.kt` рефакторено у спільний `TicketScanner`; новий `EventScanButton(eventId)` → `POST /events/:id/scan`
>   (owner-варіант `ScanTicketsButton` через `validate` збережено). EventApi: `inviteStaff/acceptStaff/scanEvent` (+ `InviteResp`/`AcceptStaffReq`).
> - **Прогнано на телефоні (LIVE, реальна камера):** організатор створив «Live Scan Test» → запросив контролера; контролер
>   `ctrl@demo.co` прийняв invite-deeplink → «✓ Ти контролер · Live Scan Test» (**DB `event_staff`**, scoped); відкрив камеру,
>   **просканував справжній QR з екрана Mac** → `POST /api/events/:id/scan` → **DB: квиток `active`→`used`, `validated_at 2026-07-02 11:31:57`**.
>   Мережа скану — через adb reverse (USB тримався); підготовлено WiFi-fallback (debug-only `network_security_config` + `-PapiBaseUrl=<lan-ip>`).
> - Побіжно розблоковано Android unit-suite (10 тестів зелені): fix pre-existing `EventDtoTest` (named-args) + stale `FormatTest`.
>
> **«Мої події як контролер» DONE (verified on device):** новий additive endpoint `GET /api/me/staff/events` (`staff::my_staff_events` —
> події owner+delegated, крім cancelled; **100% coverage**, +2 інтеграційні тести `staff_events_lists_owned_and_delegated` [SEC] +
> `staff_events_excludes_cancelled`; fmt/clippy зелені). FE: `EventApi.myStaffEvents`, `MyEventsViewModel.delegatedEvents`
> (фільтрує own), `MyEventsScreen` секція «Події, де ти контролер» з `EventScanButton` на кожній. **Прогнано на пристрої:**
> контролер `ctrl@demo.co` (без власних подій) бачить «Live Scan Test» у «Мої події» → кнопка «Сканувати квитки» — сканер
> тепер доступний завжди, без повторного invite-лінка (закрито реальний UX-розрив). Endpoint LIVE: HTTP 200, count=1.
> **Signed/rotating QR — Інкремент 1 (backend) DONE (spec-first BR-023; verified live + 100% coverage):**
> `api/src/qr.rs` — HS256 sign/verify з claims `tid`+`eid`+`exp`+`kind="qr"`, TTL `QR_TOKEN_TTL_SECS` (дефолт 120с);
> `GET /api/tickets/:id/qr` (`tickets::ticket_qr`, owner-only, лише `active`) → `{token, expires_at}`; `validate`+`scan`
> тепер приймають **signed QR** (verify підпис+exp → `tid`) АБО raw `qr_token` (backward-compat, BR-023) — існуючий скан
> не зламано. Тести: 6 unit (`qr.rs`: roundtrip, wrong-secret [SEC], non-qr-kind [SEC], expired [SEC], garbage, ttl-env)
> + 4 integration (`organizer_validates_signed_qr`, `qr_endpoint_owner_only_and_active` [SEC], `delegated_staff_scans_signed_qr`,
> `signed_qr_wrong_event_verdict`). fmt/clippy(-D)/**100% coverage** зелені. **Live curl:** mint signed JWT → validate=ok → re-scan=already_used.
> **Інкремент 2 (клієнти) — Web DONE (verified live), Android code-complete:**
> - Web: `web/lib/api.ts ticketQr` + `web/app/me/tickets` рендерить signed-QR (fetch `GET /tickets/:id/qr` → `QRCode.toDataURL(token)`),
>   auto-refresh кожні 100с (`setInterval`), сирий `qr_token` більше не використовується. **Перевірено у browser-preview:**
>   endpoint 200, згенеровано щільніший QR (JWT), 0 console-помилок, raw-токен не показується.
> - Android: `EventApi.ticketQr`+`QrResp`, `TicketsViewModel.fetchSignedQr`, `ETicketScreen` рендерить signed-QR через
>   `LaunchedEffect` loop (refresh 100с < TTL 120с) + `CircularProgressIndicator` під час завантаження; **`Text(ticket.qrToken)` прибрано**
>   (raw-токен більше не на екрані). **Перевірено на пристрої (moto_g05):** e-ticket показує щільніший QR (signed JWT) без raw-тексту
>   (порівняно з попереднім `UG-…`); **жива камера-скан ротаційного signed-QR:** контролер `ctrl` відкрив door-scanner «Live Scan Test»,
>   просканував signed-QR (241-символьний JWT) з екрана Mac → `POST /events/:id/scan` → **DB: квиток `used`, `validated_at 14:06:21`**.
>   Ротація/TTL підтверджені: протермінований токен не сканується (verify_qr→None→fallback→not_found), свіжий (< 120с) — валідний за 5с.
> **Signed-QR Інкременти 1+2 повністю DONE і перевірені (backend curl + web browser + Android device з реальною камерою).**
>
> **Інкремент 3 Part A DONE (verified live + 100% coverage):** `scan`+`validate` тепер приймають **ЛИШЕ signed-QR**
> (гілка raw → `None` → not_found/404); сирий `qr_token` на дверях більше не приймається (закрито replay-вектор).
> Тести сконвертовано raw→signed (`tickets_validate`, `staff`, `error_paths`) + додано `signed_qr_for`/`mint_qr` хелпери;
> `fmt`+`clippy -D`+**100% coverage** (4412/4412) зелені. **Live curl:** validate RAW → HTTP 404 (rejected), validate SIGNED → `ok`.
> **Part B (відкладено як micro-leaf, рекомендація):** прибрати `qr_token` з UI-DTO-відповідей — НАДЛИШКОВЕ після Part A
> (raw уже не приймається на дверях), але high-churn (ламає `tickets_reserve`-assert + клієнтські DTO + хелпери). Радимо окреме дрібне листя.
> **Залишок:** Part B (за бажанням); online anti-passback понад наявний atomic first-write.
**Created:** 2026-06-29  **Updated:** 2026-06-29

## Raw Request
> INIT-DOORSTAFF, Phase 1 (STAFF-1a). Ролі/делегування: `event_staff` + scoped
> scan-only permission; інвайт контролера (той самий signed-token seam); event-scoped
> `validate()` з вердиктами (valid/used/wrong-event/refunded); прибрати сирий `qrToken`,
> signed QR + online anti-passback. [SEC]. Декомпозуй (roles/schema, staff invite,
> validate API, Android door mode).

## Refined Description
**Scope:**
- Нова таблиця `event_staff` (event_id, user_id, perms=['scan'], invited_by) — many-to-many
  контролери на подію; серверний guard `require_perm(event_id, user, perm)`, що дозволяє
  дію organizer-власнику АБО активному запису `event_staff` з відповідним perm.
- Event-scoped валідація `POST /api/events/:event_id/tickets/validate` (заміна глобального
  `/api/tickets/validate`, старий лишається deprecated-alias — НЕ breaking), авторизація через
  `require_perm(scan)`, атомарний first-write-wins anti-passback (`UPDATE ... WHERE status='active'
  RETURNING`) і повний набір людиночитних вердиктів: `valid | already_used | wrong_event |
  refunded | expired | not_started | cancelled | not_found` (code+колір+іконка+текст).
- Signed stable QR token (HS256, той самий seam що auth-JWT; claims: `tid`, `eid`, `exp`,
  short-TTL) поверх стабільного `ticket_id`; сирий `qr_token` у DB не змінюється і НЕ покидає
  сервер у UI-відповідях; прибрати `Text(qrToken)` з Android.
- Інвайт-лінк «Контролер» на тому ж signed-token seam: `POST /api/events/:id/staff/invite` →
  single-use short-TTL токен; public preview ДО логіну; accept у 1 клік (auth-wall лише на accept);
  organizer бачить/відкликає контролерів.
- Android: екран «Сканер квитків» (organizer+staff, повноекранний вердикт-стан), «Показати квиток»
  (signed QR з авто-оновленням), «Запросити контролера» (share-sheet), екран прийняття інвайту
  (deep-link), «Мої події як контролер».

**Non-goals:**
- STAFF-1b Door Mode continuous-loop UI, STAFF-1c offline-черга сканів, STAFF-1d детальний
  scan audit-лог. TOTP/rotating-QR ротація секрету. Refund/escrow-операції на дверях (прибрані).
- Гранульовані ролі понад `scan` (`refund`/`manage` — зарезервовано в схемі, без UI/логіки).
  Веб-адмін UI керування staff. Web-клієнт (лише API + Android).

**Impacted UCs:** UC-007 (Сканування QR на вході) — суттєва зміна поведінки (делегування +
event-scoped validate + вердикти + signed QR): **new UC required (see TASK-101 spec reconciliation)**,
UC-005 (повернення квитка → вердикт `refunded`).
**Impacted BR/WF:** BR-010, BR-012 (валідація лише власником/organizer у валідному статусі →
розширити на делегованого `event_staff` з perm=scan; повторне сканування → відмова через
atomic anti-passback) — **spec-first оновлення, залежність на TASK-101**; WF-Escrow (door-flow
НЕ рухає гроші — escrow seam `api/src/escrow.rs` недоторканий); новий `event.status='cancelled'`.
**Dependencies:**
- TASK-101 — spec-reconciliation: додати новий/оновлений UC-007, розширення BR-010/BR-012,
  `event.status='cancelled'` у `docs/spec.md` ПЕРЕД кодом (rules §0/§2.2).
- TASK-002 (auth) — робочий HS256 JWT seam `api/src/auth.rs` (encode/decode, pinned Algorithm::HS256)
  переюзається для signed QR і staff-invite токенів.
- TASK-006/TASK-007 — наявні `tickets`/`validate` (`api/src/tickets.rs`), FOR UPDATE seam.
- Rate-limit/anti-abuse seam (guest/gift-шляхи) переюзати для staff-invite (INIT-ANTIABUSE).
- `api/src/telemetry.rs` — tracking-події scan_success/scan_reject/staff_invite_sent/staff_accepted.
- Deep-link інфраструктура застосунку для `/staff/invite/:token`.
**Risks / Open Questions:**
- `event.status='cancelled'` — розширення CHECK у `events`; порядок: spec → міграція → код.
- Signed-QR TTL vs UX (пропозиція 120s з авто-регеном); реген продовжує `exp`, не міняє `tid`.
- Deprecated-alias `/api/tickets/validate` — зняти після міграції Android-клієнтів (окремий cleanup).

## Estimation
**Level:** HIGH  **Justification:** MEDIUM за first-cut, але слайс перетинає **4 різні шари**
(DB/roles-schema, Rust API validate+invite, Android door mode) і **дві різні WF/BR-зміни**
(BR-010/BR-012 delegation + новий `event.status`), з обов'язковою spec-reconciliation
залежністю. За §12.5/§12.6 це декомпозується у LOW-листя перед кодом.

## Sub-tasks (DECOMPOSED)
| Sub-task ID | Title | Estimation | Status | Depends On |
| --- | --- | --- | --- | --- |
| TASK-118-A | Spec+schema: `event.status='cancelled'`, `event_staff` таблиця, `scanned_at/scanned_by`, `require_perm` guard (міграції 0033/0034) | LOW | READY | TASK-101 |
| TASK-118-B | Signed QR token seam: видача `GET /api/tickets/:id/qr` (HS256 tid/eid/exp, short-TTL), прибрати сирий `qr_token` з UI-відповідей | LOW | READY | TASK-118-A |
| TASK-118-C | Event-scoped `POST /api/events/:event_id/tickets/validate`: `require_perm(scan)`, atomic anti-passback, повний набір вердиктів; deprecated-alias старого endpoint | LOW | READY | TASK-118-A, TASK-118-B |
| TASK-118-D | Staff-invite API на signed-token seam: `POST /staff/invite`, public preview GET, `POST .../accept` (guest-signup), `GET /me/staff-events`, `GET/DELETE /events/:id/staff` | LOW | READY | TASK-118-A |
| TASK-118-E | Android door mode: екран «Сканер квитків» (вердикт-стани), «Показати квиток» (signed QR), «Запросити контролера» (share), прийняття інвайту (deep-link), «Мої події як контролер» | LOW | READY | TASK-118-B, TASK-118-C, TASK-118-D |

## Execution Order
1. TASK-101 (spec-reconciliation: UC-007/BR-010/BR-012/`cancelled`) — блокер.
2. TASK-118-A (schema/roles/guard) — фундамент.
3. TASK-118-B (signed QR seam) → TASK-118-C (event-scoped validate + anti-passback + вердикти).
4. TASK-118-D (staff-invite) — паралельно з B/C після A.
5. TASK-118-E (Android door mode) — після B, C, D.

## Speck Reference
docs/specks/SDD-118-door-staff-delegation.md
