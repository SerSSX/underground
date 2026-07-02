# TASK-116 — Не-блокуючий онбординг + contextual permission priming

**Status:** READY
**Created:** 2026-06-29  **Updated:** 2026-06-29

## Raw Request

> Онбординг має бути НЕ-блокуючим: обов'язково ≤1 екран (категорії + місто, дефолт «Київ»),
> реєстрація/повнота профілю відкладається до моменту цінності. DOB — не в онбордингу, а ленивий
> age-gate на 18+ checkout. Push/гео дозволи питаємо контекстно з pre-permission priming, а не сирим
> OS-промптом на старті. Ввести TTFW (time-to-first-wow). Android + Web. Ініціатива INIT-ONBOARDING,
> Phase 1, орієнтовна оцінка LOW/S.

## Refined Description

**Scope:**
- Скоротити онбординг до **одного обов'язкового екрана**: `categories` (multi-chip з `EVENT_CATEGORIES`) +
  `city` (autocomplete, дефолт «Київ»). «Пропустити» дозволено (порожні prefs зберігаються, `onboarded=true`),
  жоден крок не блокує перехід до Афіші. Реюз наявних `PUT /api/me/prefs` + `GET /api/me/prefs.onboarded`.
- **DOB прибрати з онбордингу.** Age-gate стає **ленивим і контекстним**: спрацьовує лише в момент дії з
  подією `age_rating>=18`. Замість глухого `409` показувати inline DOB-підказку (пікер), зберегти через
  наявний `PATCH /api/me {dob}`, потім повторити дію. Реюз `users::meets_age`.
- **Contextual permission priming** (Android + Web): гео (`near_me`/radius) і push (`POST /api/me/devices`)
  питаються **у контексті** з попереднім m-екраном «навіщо» (pre-permission), а не одразу після auth.
  Гео — при першому виборі «Поблизу» в Афіші/Discovery; push — після першої цінної дії
  (save/RSVP/claim). Якщо priming відхилено — OS-промпт НЕ показується (зберігаємо одноразовий opt-in).
- **TTFW-інструментація** через наявний telemetry-seam (`telemetry::log` / `POST /api/telemetry`):
  канонічні події `onboarding_step{step,completed}`, `permission_primer{kind,decision}`,
  `first_wow{kind}` (перший з {save | rsvp | ticket_claimed}). Нуль нових екранів у backend.

**Non-goals:**
- follow-suggestions / seed соц-графа в онбордингу (окрема ініціатива, відкладено).
- Будь-яка зміна схеми БД, нові таблиці/міграції (реюз `user_prefs`, `users.dob`, `device_tokens`).
- Реальний FCM/email pipeline (INIT-CHANNELS) — тут лише реєстрація токена + priming UX.
- Redesign Афіші/Discovery-секцій (INIT-DISCOVERY, Phase 2).
- Age-gate логіка розрахунку віку — вже існує (`users::meets_age`); не переписуємо.

**Impacted UCs:** new UC required (не-блокуючий онбординг + contextual priming + lazy age-gate) —
  див. TASK-101 (spec reconciliation): поведінка «DOB у онбордингу» та «дозволи сирим OS-промптом»
  змінюється, тож `docs/spec.md` оновлюється ПЕРЕД тестами й кодом (rules §2.2). Дотичні наявні:
  A.1 Auth-навігація (новий user → Onboarding), A.2 Onboarding, A.3 Discovery (гео-priming), A.5 Checkout (age-gate).

**Impacted BR/WF:**
- WF-Onboarding: 3 кроки → 1 обов'язковий екран; DOB/follow/push винесені з лінійного флоу.
- BR-AgeGate: `meets_age` виклик лишається; UX-гілка «немає DOB» → inline-підказка замість `409`-fail.
- Жодного дотику до escrow/payments seam (онбординг грошей не рухає).

**Dependencies:**
- TASK-101 (spec reconciliation) — фіксація нової поведінки у `docs/spec.md` ПЕРЕД кодом.
- INIT-BROWSE-AUTH (browse-before-auth / persist anon-intent) — онбординг слідує за auth-навігацією;
  не блокує цю задачу, але узгодити точку входу «новий user → Афіша, не Onboarding-wall».
- Реюз наявних seam'ів: `prefs.rs`, `users.rs` (`meets_age`, `PATCH /api/me`), `devices.rs`, `telemetry.rs`.

**Risks / Open Questions:**
- Google Play/App Store вимоги до pre-permission: priming-екран не має імітувати системний діалог (uplift-copy).
- Гео на Web = browser Geolocation API (потрібен HTTPS + user-gesture); fallback на `city`-фільтр без гео.
- Ризик подвійного OS-промпту push, якщо priming-стан не персиститься локально (client-side flag).
- Відкрите: чи потрібен окремий telemetry-property `source` для `first_wow` (узгодити з INIT-ANALYTICS Tracking Plan v1).

## Estimation

**Level:** MEDIUM (flag+confirm, rules §12.5)
**Justification:** Роадмап оцінює INIT-ONBOARDING як S/LOW, і схема/БД не змінюються (реюз наявних
  endpoints). Проте зміна перетинає **≥3 шари/клієнти** (Android UI, Web UI, backend telemetry-touch)
  і **два distinct surfaces priming** (гео + push) плюс lazy-age-gate у Checkout — це виходить за межі
  одного LOW-листа. За §12.5 позначаю MEDIUM і декомпоную на LOW-листя ПЕРЕД кодом (див. нижче).
  Немає нового escrow/payments/schema-ризику → не HIGH.

## Планова декомпозиція (LOW-листя перед кодом, §12.5/§12.6)

> Межі декомпозиції: distinct layer (API/Android/Web) × distinct WF (onboarding-lite / age-gate / gео-priming / push-priming / telemetry).
> Код не починається, доки всі листя не LOW і speck-audit не пройдено.

| Sub-task ID | Title | Estimation | Status | Depends On |
|---|---|---|---|---|
| TASK-116-A-API | Telemetry constants `onboarding_step`/`permission_primer`/`first_wow` + reuse `prefs`/`meets_age` (нуль схеми) | LOW | READY | TASK-101 |
| TASK-116-B-DROID-ONB | Android: 1-екранний non-blocking онбординг (categories+city, дефолт Київ, Skip) через `PUT /api/me/prefs` | LOW | READY | TASK-116-A-API |
| TASK-116-C-WEB-ONB | Web `/onboarding`: 1-екранна non-blocking форма (categories+city) + `onboarded`-guard | LOW | READY | TASK-116-A-API |
| TASK-116-D-AGEGATE | Lazy age-gate UX (Android+Web): inline DOB-пікер при 18+ дії → `PATCH /api/me {dob}` → retry, замість `409`-fail | LOW | READY | TASK-116-A-API |
| TASK-116-E-GEO-PRIME | Contextual гео-priming (Android+Web) при виборі «Поблизу»: pre-permission → native prompt; fallback city-filter | LOW | READY | TASK-116-B-DROID-ONB, TASK-116-C-WEB-ONB |
| TASK-116-F-PUSH-PRIME | Contextual push-priming (Android) після першої цінної дії → `POST /api/me/devices`; persist opt-in flag | LOW | READY | TASK-116-B-DROID-ONB |

## Execution Order

1. TASK-101 (spec reconciliation) — фіксація нової поведінки у `docs/spec.md`.
2. TASK-116-A-API (telemetry constants seam) — контракт подій для всіх клієнтів.
3. TASK-116-B-DROID-ONB ∥ TASK-116-C-WEB-ONB (1-екранний онбординг на обох клієнтах).
4. TASK-116-D-AGEGATE (lazy age-gate) — незалежний від 2/3, після A.
5. TASK-116-E-GEO-PRIME (після онбордингу обох клієнтів).
6. TASK-116-F-PUSH-PRIME (після Android-онбордингу; потребує «першої цінної дії»).

## Speck Reference

docs/specks/SDD-116-onboarding-lite.md
