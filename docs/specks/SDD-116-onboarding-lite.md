# SDD-116 — Не-блокуючий онбординг + contextual permission priming

**Impacted UCs:** new UC required (non-blocking onboarding + contextual permission priming + lazy age-gate) —
  spec reconciliation через TASK-101; дотичні наявні A.1 Auth-нав, A.2 Onboarding, A.3 Discovery, A.5 Checkout.
**Impacted BR/WF:** WF-Onboarding (3 кроки → 1 обов'язковий екран); BR-AgeGate (`users::meets_age`
  лишається; UX-гілка «немає DOB» → inline-підказка замість `409`); нуль дотику escrow/payments seam.

## Scope / Non-goals

**Scope:**
- ≤1 обов'язковий екран онбордингу (categories + city, дефолт «Київ», Skip-дозволено), non-blocking навігація до Афіші.
- Lazy age-gate: inline DOB-пікер у момент 18+ дії замість глухого `409`; реюз `PATCH /api/me {dob}` + `users::meets_age`.
- Contextual permission priming (гео + push): pre-permission екран «навіщо» ПЕРЕД OS-промптом; відмова → OS-промпт не показується.
- TTFW-телеметрія через наявний seam (`telemetry::log` / `POST /api/telemetry`): `onboarding_step`, `permission_primer`, `first_wow`.

**Non-goals:** follow-suggestions в онбордингу; будь-яка зміна схеми/міграції; реальний FCM/email pipeline (INIT-CHANNELS);
  redesign Discovery (INIT-DISCOVERY); переписування розрахунку віку.

**Реюзовані модулі:** `api/src/prefs.rs` (`PUT /api/me/prefs`, `GET /api/me/prefs`), `api/src/users.rs`
  (`PATCH /api/me`, `meets_age`), `api/src/devices.rs` (`POST /api/me/devices`), `api/src/telemetry.rs`
  (`log`, `POST /api/telemetry`); `android/.../ui/onboarding/**`, `android/.../ui/afisha/**`, `android/.../ui/detail/**`;
  `web/app/onboarding/**`, `web/app/events/**`. Провайдер-нейтральний escrow seam НЕ торкається (онбординг грошей не рухає).

## Acceptance Criteria

- AC-1: Онбординг рендерить **рівно один** обов'язковий екран з полями `categories` (multi-chip з `EVENT_CATEGORIES`)
  і `city` (autocomplete, попередньо заповнено «Київ»). Жодного окремого DOB/follow/push-кроку у лінійному флоу.
- AC-2: Кнопка «Пропустити» на онбординг-екрані зберігає порожні prefs і встановлює `onboarded=true`
  (через `PUT /api/me/prefs` з `categories:[]`), після чого користувач потрапляє на Афішу; повторний вхід
  не повертає на онбординг (guard за `GET /api/me/prefs.onboarded`).
- AC-3: Збереження prefs викликає `PUT /api/me/prefs {categories, city}`; `city` порожній/whitespace нормалізується
  до null; `categories` понад `MAX_CATEGORIES` відхиляється валідацією (реюз наявної) з читабельною помилкою.
- AC-4: Дія з подією `age_rating>=18` для користувача **без** `dob` НЕ повертає глухий `409` у UI —
  показується inline DOB-пікер; після `PATCH /api/me {dob}` дія автоматично повторюється; при достатньому
  віці — успіх, при недостатньому — типізована відмова «вік не відповідає» (не бекенд-`500`).
- AC-5: Гео-priming: тап «Поблизу»/radius у Discovery спершу показує pre-permission екран «навіщо гео»;
  лише «Дозволити» запускає OS/browser-геозапит. «Не зараз» → фолбек на `city`-фільтр, OS-промпт не викликається.
- AC-6: Push-priming (Android): після першої цінної дії (save|rsvp|claim) показується pre-permission екран;
  «Увімкнути» → OS-промпт → на grant `POST /api/me/devices {token, platform}`; «Пізніше» → OS-промпт не викликається,
  стан persist локально (повторно не спамить того ж сеансу).
- AC-7: Телеметрія: `onboarding_step{step, completed}` емітиться на save/skip; `permission_primer{kind, decision}`
  на кожне рішення priming; `first_wow{kind}` рівно раз на перший з {save|rsvp|ticket_claimed}. Емісія
  best-effort (падіння telemetry не блокує UX — реюз інфалібельного `telemetry::log`).
- AC-8: Онбородинг і priming не викликають жодного escrow/payments seam і не мутують `orders`/`tickets`/`escrow`
  (перевіряється відсутністю викликів у code-review + інтеграційним тестом на нуль записів у ledger).

## Security Acceptance Criteria

- SEC-1 (authz / IDOR): `PUT /api/me/prefs`, `PATCH /api/me`, `POST /api/me/devices` діють **лише** над
  `CurrentUser` з JWT; жоден endpoint не приймає `user_id` з тіла/query. Спроба записати prefs/dob/token
  іншому користувачу неможлива (self-scope), негативний тест підтверджує.
- SEC-2 (injection): `categories`, `city`, `dob`, `token`, `platform` йдуть через параметризований SQLx
  (`bind`), без інтерполяції; oversized/шкідливі рядки (SQL-meta, control-chars) не змінюють запит і
  відхиляються валідацією або зберігаються безпечно як текст.
- SEC-3 (oversized / DoS): `categories` обмежено `MAX_CATEGORIES`; `city`/`token` обмежено довжиною;
  надмірний payload → `400 Validation`, не паніка/OOM.
- SEC-4 (token-replay / device-hijack): `POST /api/me/devices` `ON CONFLICT(token)` переприв'язує токен до
  поточного `CurrentUser` — вкрадений/чужий FCM-токен не лишається у попереднього власника; жоден device
  не адресується без owner-JWT.
- SEC-5 (rate-limit): повторні виклики `PUT /api/me/prefs` / `POST /api/telemetry` (анонімна емісія)
  не дозволяють enumeration/flood — застосовується наявний rate-limit-шар; телеметрія best-effort і не
  розкриває стан інших користувачів.
- SEC-6 (leak): pre-permission екран НЕ логує сирих гео-координат/FCM-токена в telemetry props; `permission_primer`
  несе лише `{kind, decision}`; `first_wow` — лише `{kind}`. Жодного PII/секрету в telemetry payload.
- SEC-7 (age-gate bypass): клієнтський inline DOB-пікер — UX-зручність; авторитетна перевірка лишається
  серверною (`users::meets_age` у Checkout-шляху). Підроблений клієнтський `dob`/пропуск пікера не обходить
  серверний gate 18+ дії.

## Failure Modes / Error Mapping

| Ситуація | Поведінка | Код/UX |
|---|---|---|
| `PUT /api/me/prefs` з >MAX categories | валідація відхиляє | `400 Validation`, inline-помилка чипів |
| `city` невідоме газетиру | `cities::resolve` → country null, prefs зберігаються | `200`, без блокування |
| 18+ дія, немає `dob` | inline DOB-пікер, retry після `PATCH` | не `409`-fail; після пікера — повтор |
| 18+ дія, `dob` недостатній | типізована відмова | «Подія 18+, вік не відповідає» (не `500`) |
| Гео-priming «Не зараз» | фолбек city-filter | OS-промпт НЕ викликається |
| Push-priming «Пізніше» | локальний persist, без OS-промпту | повторно не спамить |
| Grant push, `token` порожній | валідація | `400 Validation`, без реєстрації |
| Telemetry insert падає | best-effort, лог warn | UX не блокується (інфалібельний `log`) |
| Відсутній JWT на me-endpoint | auth-reject | `401 Unauthorized` |

## Test Matrix

| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 (1 екран) | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ | ⬜ |
| AC-2 (skip → onboarded) | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ⬜ |
| AC-3 (put prefs valid) | ✅ | ✅ | ✅ | ⬜ | ✅ | ⬜ | ⬜ |
| AC-4 (lazy age-gate) | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ⬜ |
| AC-5 (гео-priming) | ⬜ | ⬜ | ⬜ | ✅ | ✅ | ⬜ | ⬜ |
| AC-6 (push-priming) | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ⬜ |
| AC-7 (telemetry TTFW) | ✅ | ✅ | ✅ | ⬜ | ✅ | ⬜ | ⬜ |
| AC-8 (no escrow touch) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-1 (authz/IDOR) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| SEC-2 (injection) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-3 (oversized) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-4 (token-replay) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-5 (rate-limit) | ⬜ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| SEC-6 (leak props) | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ |
| SEC-7 (age-gate bypass) | ✅ | ✅ | ✅ | ⬜ | ✅ | ✅ | ⬜ |

> UI-стадії (Base UI / UI) застосовні лише до клієнтських листів (Android + Web) — Playwright device matrix
> для web-онбордингу/age-gate/priming; Compose UI-тести для Android. Curl Prod / Prod Stack — smoke на
> me-endpoints і age-gate у продовому стеку. Container-first, 100% unit+integration coverage,
> positive + negative + [SEC] вектори (rules §6/§7).
