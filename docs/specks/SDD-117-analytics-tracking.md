# SDD-117 — Tracking plan v1 + purchase-воронка + K-factor атрибуція + north-star дашборд

**Impacted UCs:** new UC required — UC-«Product analytics & funnel», UC-«K-factor attribution» (фіксуються у `docs/spec.md` через TASK-101, INIT-ROADMAP-SPEC, ПЕРЕД кодом — rules §0/§2.2). Дотичні існуючі UC: перегляд події, checkout/paid, QR-scan (attend), guest-claim.
**Impacted BR/WF:** BR north-star (квитки paid+free/RSVP на активного/28д), K-factor ≥ 0.5, пороги релізу share→claim ≥ 30% / guest→signup ≥ 40% (roadmap §9.1–§9.2). WF purchase-funnel `view→detail_open→tier_select→checkout_start→paid→ticket_claimed→attend`; viral-loop `share/invite/gift→claim→guest_signup→re_share`.

## Scope / Non-goals
**Scope:**
- Канонічний Tracking Plan v1 (~20 подій) з фіксованою property-схемою і ЄДИНИМ point-of-emit; розширення наявного `api/src/telemetry.rs::log` seam і `POST /api/telemetry`; реюз таблиці `telemetry_events` (0016) — без нових event-таблиць і без нових продуктових екранів.
- Серверна purchase-воронка з drop% + timeseries + сегментація `source/channel`: `GET /api/admin/telemetry/funnel`.
- K-factor через `share_token`/`token_nonce` з парною атрибуцією `*_sent ↔ *_accepted` + `attributed_signup`: `GET /api/admin/telemetry/kfactor`.
- North-star дашборд у `admin/app/telemetry` (read-only).
- Точки емісії: Android (`android/.../ui/**`), public web (`web/app/**`) — best-effort клієнтські події; серверні події інлайн у хендлерах (`orders.rs`,`tickets.rs`,`payments.rs`,`webhooks.rs`,`eventsocial.rs`,`auth.rs`).

**Non-goals:**
- Не чіпати `api/src/escrow.rs` (provider-neutral NoopProvider); гроші не рухаються.
- Не створювати gift/share-token seam (постачають INIT-GIFTLINK/INIT-DEEPLINKS) — тут лише **читання** `token_nonce`/`share_token` як атрибуційного ключа; raw token у props ЗАБОРОНЕНО.
- Не CRM/lifecycle-тригери; не A/B-платформа; не нові продуктові екрани.

## Canonical Tracking Plan v1 (~20 подій, ЄДИНИЙ point-of-emit)
`event_view{event_id,source}` · `detail_open{event_id,source}` · `tier_select{event_id,tier_id}` · `checkout_start{event_id,tier_id,qty,amount_minor,is_guest}` · `checkout_paid{event_id,order_id,amount_minor}` · `checkout_failed{event_id,reason}` · `ticket_claimed{event_id,token_nonce,has_account}` · `rsvp{event_id,state}` · `scan_result{event_id,verdict}` · `gift_sent{event_id,token_nonce}` ↔ `gift_accepted{event_id,token_nonce}` · `invite_sent{event_id,channel,token_nonce}` ↔ `invite_accepted{event_id,token_nonce}` · `share_clicked{surface,event_id}` · `re_share{event_id,token_nonce}` · `guest_signup{token_nonce,attributed:bool}` · `attributed_signup{referrer_id,token_nonce}` · `onboarding_step{step,completed}` · `app_open{}` · `session_start{}`. Props: 3–6/подію; лише id/enum/nonce-hash — жодного raw email/token.

## Acceptance Criteria
- **AC-1:** Існує машиночитний контракт подій (Rust const allow-list у `telemetry.rs`): `POST /api/telemetry` приймає ЛИШЕ ім'я зі списку; невідоме ім'я → 422 Validation, у БД не пишеться.
- **AC-2:** Кожна серверна подія (`signup`,`checkout_start`,`checkout_paid`,`rsvp`,`ticket_claimed`,`scan_result`) емітиться інлайн у своєму єдиному хендлері через `telemetry::log`; збій запису телеметрії НЕ ламає основну дію (best-effort, лишається інфалібним для caller).
- **AC-3:** `GET /api/admin/telemetry/funnel` повертає впорядковані кроки `view→detail_open→tier_select→checkout_start→paid→ticket_claimed→attend` з абсолютним count і drop% між сусідніми кроками; підтримує `?from&to` (timeseries-бакети) і `?segment=source|channel`.
- **AC-4:** `attend` рахується як `scan_result{verdict=ok}`; крок присутній у воронці навіть за нуля сканів (0 count, а не відсутній).
- **AC-5:** `GET /api/admin/telemetry/kfactor` обчислює K-factor = `invites_accepted / active_users` за вікном `?days` (дефолт 28) через парну атрибуцію `*_sent ↔ *_accepted` за спільним `token_nonce`; повертає також share→claim% і guest→signup%.
- **AC-6:** `attributed_signup` мостить анонімний `anon_id` до `user_id` при першому логіні; повторна атрибуція того самого `token_nonce` не подвоює лічильник (idempotent за nonce).
- **AC-7:** Android емітить `event_view`,`detail_open`,`tier_select`,`checkout_start`,`share_clicked`,`invite_sent`,`onboarding_step` через `POST /api/telemetry`; помилка мережі не блокує UI-дію.
- **AC-8:** Web (public) емітить `event_view`,`detail_open`,`share_link_visited`,`guest_signup`,`re_share` з детермінованим `anon_id` (cookie/localStorage), що переживає навігацію ДО логіну.
- **AC-9:** Admin дашборд `admin/app/telemetry` показує NSM-картку (квитки/активного/28д), воронку з drop%, K-factor, channel-breakdown і timeseries; усі числа з funnel/kfactor endpoint-ів.
- **AC-10:** Жодна нова event-таблиця не створюється: усе пише у `telemetry_events` (0016). Якщо агрегати funnel повільні — додається лише індекс (міграція 0033), без зміни схеми емісії.
- **AC-11:** Поведінкові визначення (NSM, K-factor, пороги) відповідають `docs/spec.md` після TASK-101; SDD не вводить чисел, яких немає у spec (spec-is-truth, §2.2).

## Security Acceptance Criteria
- **SEC-1 (authz / IDOR):** `GET /api/admin/telemetry/funnel` і `/kfactor` вимагають `CurrentAdmin`; звичайний `CurrentUser` або анонім → 401/403; жоден admin-endpoint не приймає `user_id`-параметр, що дозволив би читати чужі сирі події.
- **SEC-2 (rate-limit / abuse):** Публічний `POST /api/telemetry` має per-IP і per-`anon_id` rate-limit; перевищення → 429 без запису; лічильник не інфлятиться флудом (захист K-factor/NSM від накрутки).
- **SEC-3 (injection):** `name` валідність — лише allow-list; `props` парситься як JSON і пишеться параметризовано (bind), не конкатенується в SQL; нестандартний `name` (SQL/JSON-injection payload) → 422.
- **SEC-4 (oversized):** `props` > ліміту (напр. 4 KB) або `name` > 64 → 422; захист від oversized-body та cardinality-bomb.
- **SEC-5 (token-replay / leak):** У `props` пишеться лише `token_nonce` (hash), НІКОЛИ raw `share_token`, email чи magic-code; неможливо реплеїти share/gift-token з телеметрії; `attributed_signup` не приймає raw token.
- **SEC-6 (PII / GDPR):** `telemetry_events` включений у GDPR-erasure (`api/src/gdpr.rs`): при видаленні користувача `user_id` анонімізується; guest-props не містять прямих ідентифікаторів (координація з INIT-COMPLIANCE-GUARDS).
- **SEC-7 (money-seam integrity):** Жоден tracking-код не викликає і не мутує `escrow.rs`/payout-стан; емісія `checkout_paid` читає, але не змінює order/escrow.

## Failure Modes / Error Mapping
| Умова | Код / поведінка |
|---|---|
| Невідоме/порожнє `name`, `name` > 64 | 422 Validation, no write |
| `props` > 4 KB | 422 Validation, no write |
| Rate-limit перевищено (IP/anon_id) | 429 Too Many Requests, no write |
| Не-admin на `/funnel` \| `/kfactor` | 401 (анонім) / 403 (user) |
| Збій INSERT телеметрії | `tracing::warn`, дія-носій успішна (best-effort) |
| Порожнє вікно funnel (нема подій) | 200 з нулями + усіма кроками (не 404) |
| Дубль `attributed_signup` (той самий nonce) | idempotent — лічильник не росте |
| Raw token у props (баг емітера) | відкидається валідатором props → 422 (SEC-5) |

## Test Matrix
| AC / SEC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 allow-list contract | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-2 server point-of-emit (best-effort) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-3 funnel drop%/timeseries/segment | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-4 attend=scan ok (zero-safe) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-5 K-factor + share→claim/guest→signup | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-6 anon→user merge idempotent | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-7 Android emit best-effort | ✅ | ✅ | ⬜ | ✅ | ✅ | ⬜ | ✅ |
| AC-8 Web emit + anon_id persist | ✅ | ✅ | ⬜ | ✅ | ✅ | ⬜ | ✅ |
| AC-9 admin dashboard render | ⬜ | ✅ | ⬜ | ✅ | ✅ | ⬜ | ✅ |
| AC-10 no new event-table / reuse | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-11 defs match spec (post TASK-101) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-1 authz/IDOR | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-2 rate-limit 429 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-3 injection → 422 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-4 oversized → 422 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-5 token/PII leak guard | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-6 GDPR-erasure telemetry | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-7 escrow untouched | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |

**Legend:** ✅ обов'язкова стадія; ⬜ незастосовно. UI-стадії (Base UI / UI, Playwright device-matrix §7) — лише для AC з клієнтською поверхнею (Android AC-7, Web AC-8, Admin AC-9). Container-first, 100% unit+integration, кожна сюїта positive + negative + [SEC] (§6).
