# SDD-102 — Незворотні контракти: minor-units гроші, event_timezone, i18n-каркас
**Impacted UCs:** new UC required (event_timezone на write/read + tz-aware презентація часу; minor-units контракт платіжного seam) — узгодити у spec-reconciliation **TASK-101**; дотичні наявні UC перегляду/створення події та платіжного DTO отримують адитивне non-breaking поле.
**Impacted BR/WF:** новий BR-money (усі гроші API/ledger = `amount_minor:i64`+`currency`, конверсія лише в `money.rs`); новий BR-tz (кожна подія MUST мати валідний IANA `event_timezone`, час презентується в поясі події); WF create/update event резолвить `event_timezone` з міста з дозволеним IANA-override.

## Scope / Non-goals
**Модулі (real):** `api/src/money.rs` (новий), `api/src/currency.rs`, `api/src/cities.rs`, `api/src/events.rs`, `api/migrations/0033_event_timezone.sql`, `api/Cargo.toml` (chrono-tz); Android `android/app/build.gradle.kts`, `.../util/Format.kt`, `.../data/Event.kt`, `.../ui/**` (EventCard/EventDetailScreen/CreateEventScreen), `.../res/values/strings.xml`, `.../res/values-en/strings.xml`; Web `web/lib/i18n.tsx`, `web/lib/api.ts`, `web/app/**` (EventCard/EventDetailClient/me/tickets/create-event); Docs `docs/spec.md`, `docs/rules.md`.

**Scope:**
- **S1 CONTRACT-MONEY:** канон `amount_minor:i64`+`currency` на всіх нових/змінюваних wire-DTO платіжного seam; ЄДИНА точка конверсії `NUMERIC(12,2)↔minor` у `money.rs`; per-currency exponent у `currency.rs`.
- **S2 CONTRACT-TZ:** міграція 0033 (`events.event_timezone` + `cities.timezone` + backfill + NOT NULL DEFAULT); DTO-розширення; IANA-валідація на write.
- **S3 FIX-RENDER:** tz-aware рендер часу Android (`java.time`) і Web (`Intl`) + суфікс «за місцевим»/«local».
- **S4 I18N-SCAFFOLD:** `values-en/strings.xml`; нові Android-рядки лише через `stringResource`.
- **S5 GUARDRAIL+SPEC:** rules guardrail + spec фіксація контрактів.

**Non-goals:** реальний PSP/провайдерна інтеграція (Фаза 4); масова заміна 306 наявних кирилиць (TASK-I18N-DEBT); повний `ledger_entries` double-entry (TASK-LEDGER-1); зміна типу `events.price`→BIGINT; wallet/checkout UI; event_sessions/multi-day; DM/feed/discovery; фоновий tz-recompute уже проданих квитків (starts_at незмінні — лише presentation).

**Payments (provider-neutral):** жодна логіка не торкається `api/src/escrow.rs`/провайдера; лише ФІКСУЄТЬСЯ тип `amount_minor:i64`, який споживають TASK-LEDGER-1/TASK-PROV-1. Активного PSP немає (product-first, payment last).

## Acceptance Criteria
- **AC-1:** Подія в Києві (`event_timezone='Europe/Kyiv'`, `starts_at=17:00Z`) на Android і Web показує `19:00` улітку (+3) та `18:00` взимку (+2), а не `17:00`; на пристрої з іншим поясом додається суфікс «за місцевим»/«local».
- **AC-2:** `money::to_minor(Decimal('650.00'),'UAH')==65000`; `from_minor(65000,'UAH')==Decimal('650.00')`; `to_minor(Decimal('650'),'JPY')==650`; round-trip ідемпотентний для всіх gazetteer-валют.
- **AC-3:** `currency::exponent` повертає `0` для JPY/KRW, `2` для UAH/USD/EUR/GBP/PLN та решти; тест перебирає кожну валюту з `for_country`.
- **AC-4:** Міграція 0033 backfill проставляє `event_timezone` для 100% наявних `events` (жоден NULL); нова подія без явного поясу успадковує пояс міста; невідоме місто → `Europe/Kyiv`.
- **AC-5:** `POST/PATCH /api/events` з невалідним `event_timezone` (напр. `Mars/Phobos`) → `400 invalid_timezone`; валідний IANA приймається і повертається.
- **AC-6:** `EventDto` завжди містить `event_timezone:String` (non-null) у JSON; старі клієнти десеріалізують без падіння (адитивне поле).
- **AC-7:** `coreLibraryDesugaring` увімкнено; `Format` не містить `iso.substring(11,16)`; `fmtDateL` не містить `iso.slice(11,16)`; grep по кодовій базі не знаходить substring/slice-рендеру часу.
- **AC-8:** `values-en/strings.xml` існує і містить усі нові ключі задачі (`time_local`, `event_timezone_label` тощо); нові рядки в UI-коді — через `stringResource`, не літерал.
- **AC-9:** `docs/spec.md` зафіксовано (гроші=minor-units i64 на API/ledger; кожна подія має `event_timezone`; час показується в поясі події); `docs/rules.md` містить guardrail проти substring-рендеру часу та вимогу `stringResource` для нових Android-рядків.
- **AC-10:** wire-DTO платіжного seam (Charge/Payout/Refund/Webhook) серіалізують `amount_minor:i64`+`currency`, НЕ Decimal; domain-код далі рахує в Decimal, конверсія лише в `money.rs` на межі.
- **AC-11:** Явний валідний override `event_timezone='Europe/Warsaw'` зберігається і повертається у DTO; `GET /api/cities?q=` повертає `timezone` для клієнтського прев'ю поясу.

## Security Acceptance Criteria
- **SEC-1 (injection/whitelist-parse):** `event_timezone` валідується whitelist-парсером (`chrono_tz Tz::from_str`), не потрапляє сирим у форматування чи SQL; невалідне → `400`, не `500` і не збережене. Вектор з SQL-мета-символами (`'; DROP…`) відхиляється до збереження.
- **SEC-2 (money precision/overflow):** `to_minor` перевіряє, що `Decimal` має ≤`exponent` знаків після коми (інакше `Err`, не тихе округлення) і що результат вміщується в `i64` (checked множення на `10^exponent`); запобігає втраті грошей та overflow-отруєнню ledger.
- **SEC-3 (negative/NaN money):** `to_minor` відхиляє від'ємні/NaN суми там, де контракт вимагає `≥0` (charge/payout amount), уніфіковано на межі, не в кожному викликачі.
- **SEC-4 (DST-DoS/ambiguity):** рендер не кидає виняток на межах DST (`java.time`/`Intl` обробляють коректно); `02:30` у ніч переведення годинників на обох платформах не падає.
- **SEC-5 (backfill-integrity/leak-safety):** міграція 0033 не лишає жодного NULL `event_timezone` і НЕ змінює наявних `starts_at/ends_at` (UTC-інстанти незмінні — змінюється лише presentation), щоб не зсунути вже продані квитки; повторний прогін idempotent-safe.
- **SEC-6 (i18n-injection):** жоден `stringResource`/en-рядок не форматується з неекранованим user-input у HTML/лог; нові en-рядки не містять format-string вразливостей.
- **SEC-7 (contract-tamper/leak):** клієнт не може підмінити `currency` для отримання іншого exponent на межі конверсії; невідома валюта → `Err`, не дефолтний exponent, що замовчувано спотворює суму.

## Failure Modes / Error Mapping
| Умова | Поведінка |
|---|---|
| Невалідний IANA `event_timezone` на write | `400 invalid_timezone` (не 500, не збережено) |
| Невідоме місто при резолві | fallback `Europe/Kyiv`, `event_timezone` завжди non-null |
| `to_minor` надлишок знаків (`650.001` при exp 2) | `Err precision` — НЕ тихе округлення |
| `to_minor` overflow (> i64) | `Err` (checked) — ledger не отримує зіпсоване значення |
| `to_minor` від'ємна/NaN у charge-контексті | `Err` — уніфіковано на межі |
| `currency::exponent` невідома валюта | `Err` — не дефолтний exponent |
| `from_minor(i64::MAX,...)` | не панікує, детермінований `Decimal` |
| DST-неіснуюча локальна година | рендериться без винятку (`java.time`/`Intl`) |
| Старий клієнт на write без `event_timezone` | резолв із city (не падіння) |
| Повторний прогін міграції 0033 | idempotent — не змінює вже проставлені пояси й `starts_at` |

## Test Matrix
| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 (tz render 19:00/18:00 + суфікс) | ✅ | ✅ | ⬜ | ✅ | ✅ | ⬜ | ⬜ |
| AC-2 (to/from_minor round-trip) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-3 (exponent per-currency) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-4 (backfill 100% non-null) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-5 (invalid tz → 400) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| AC-6 (EventDto завжди tz non-null) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| AC-7 (no substring/slice render) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-8 (values-en + stringResource) | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ | ⬜ |
| AC-9 (spec+rules guardrail) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-10 (wire-DTO amount_minor) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-11 (tz override + city.timezone) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-1 (tz injection) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ⬜ |
| SEC-2 (precision/overflow) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-3 (negative/NaN) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-4 (DST 02:30 no-crash) | ✅ | ⬜ | ⬜ | ✅ | ✅ | ⬜ | ⬜ |
| SEC-5 (backfill integrity/idempotent) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-6 (i18n-injection) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-7 (currency tamper) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ |

**Стадії:** UI/Base-UI лише для tz-презентації часу, суфікса «за місцевим»/«local», DST-рендеру та scaffold-рядків (Playwright device-matrix для Web EventCard/EventDetail; Android Compose/instrumented для Format+EventCard). Curl Dev/Prod — для API-контрактів tz/DTO. Prod Stack — для незворотної цілісності backfill 0033. Кожна сюїта: positive + negative + [SEC] вектори, container-first, 100% unit+integration coverage нового коду (rules §6/§7).
