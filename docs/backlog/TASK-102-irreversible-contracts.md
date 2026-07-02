# TASK-102 — Незворотні контракти: minor-units гроші, event_timezone, i18n-каркас
**Status:** IN PROGRESS (BE-контракти DONE)
**Created:** 2026-06-29  **Updated:** 2026-07-01

> **Реалізовано (2026-07-01) + verified (`cargo test`, контейнерна БД §8.6):**
> - **Money (§3.2 drift-реконсиляція):** кодова база вже на `Decimal`/`NUMERIC(12,2)`, **не** float →
>   ризикована i64-міграція визнана НЕпотрібною; BR-019 у `docs/spec.md` виправлено під реальність (precise Decimal, ніколи float).
> - **event_timezone (BR-020):** міграція `api/migrations/0033_event_timezone.sql` (`timezone TEXT NOT NULL DEFAULT 'Europe/Kyiv'`,
>   backfill через DEFAULT); `api/src/events.rs` — поле в `Event`/`EVENT_COLS`/create/update DTO, валідація IANA через
>   `chrono-tz` (нова dep, justified §9.2), default `Europe/Kyiv`; 3 нові тести (default→Kyiv, valid IANA, invalid→422)
>   + 0 регресій у discover/events_manage/events_repo/events_search/eventsocial (25 тестів зелені).
>
> **Залишок (LOW-листя, FE):** показ часу події **в поясі події** на Android/Web (BR-020 display-side);
> перевірка i18n-каркасу (web `web/lib/i18n.tsx` вже є; Android strings). Повний `./scripts/ci.sh` — перед фінальним DONE.

## Raw Request
> «Незворотні контракти: minor-units гроші, event_timezone, i18n-каркас» (ініціатива INIT-CONTRACTS, Phase 0). Дешеві незворотні технічні контракти ПЕРЕД фічами: гроші в minor units (int) скрізь; events.timezone + коректний рендер часу (Android/Web); i18n-каркас (uk/en) без повного перекладу. Міграції + backfill. Декомпозуй на LOW-листя (schema, API, Android, Web).

## Refined Description
**Scope:**
- **CONTRACT-MONEY:** канон грошей `amount_minor:i64` + `currency` на ВСІХ нових/змінюваних wire-DTO платіжного домену (Charge/Payout/Refund/Webhook) і на майбутньому `ledger_entries.amount_minor:BIGINT`. Конвертація `NUMERIC(12,2)↔minor` інкапсульована в ЄДИНОМУ seam `api/src/money.rs` (`to_minor(Decimal,&str)->Result<i64>`, `from_minor(i64,&str)->Decimal`), що читає per-currency decimal-exponent з `api/src/currency.rs` (`exponent(code)->u32`: JPY/KRW=0, решта=2). Домен далі рахує в `Decimal`; конверсія лише на межі wire/ledger.
- **CONTRACT-TZ:** міграція `0033_event_timezone.sql` — `events.event_timezone TEXT` (IANA) + `cities.timezone TEXT`; backfill із gazetteer (0015) для 100% рядків; `NOT NULL DEFAULT 'Europe/Kyiv'` після backfill. `EventDto += event_timezone:String`; `CreateEventReq/UpdateEventReq += event_timezone:Option<String>` (валідація IANA через `chrono_tz Tz::from_str`); `City += timezone`.
- **FIX-RENDER:** Android `util/Format.kt` через `java.time` (`coreLibraryDesugaring`) замість `iso.substring(11,16)`; Web `web/lib/i18n.tsx fmtDateL` через `Intl.DateTimeFormat({timeZone})` замість `iso.slice(11,16)`. Суфікс «за місцевим»/«local» коли пояс події ≠ пояс пристрою.
- **I18N-SCAFFOLD:** `android/app/src/main/res/values-en/strings.xml` (паралель до `values/strings.xml`); НОВІ рядки цієї задачі — лише через `stringResource`. Web i18n-каркас (`uk/en`) розширюється, не створюється з нуля.
- **RULES+SPEC:** `docs/rules.md` guardrail (заборона `substring/slice`-рендеру часу; start/end layout; нові Android-рядки лише через `stringResource`); `docs/spec.md` фіксує гроші=minor-units i64 на API/ledger, кожна подія має `event_timezone`, час показується в поясі події.

**Non-goals:**
- Реальний PaymentProvider (Stripe/LiqPay/LS) — Фаза 4 (тут лише КОНТРАКТ типу, не інтеграція).
- Масова заміна 306 наявних хардкод-кирилиць Android → `stringResource` — окремий debt-таск (TASK-I18N-DEBT); тут лише каркас `values-en` + правило для нових рядків.
- Повний `ledger_entries` double-entry запис — TASK-LEDGER-1; тут лише узгодження типу.
- Зміна БД-типу `events.price` з NUMERIC на BIGINT — НЕ робимо (менший blast-radius).
- Wallet/Apple-Google Pay, checkout UI, event_sessions/multi-day, DM/feed/discovery — інші ініціативи.

**Impacted UCs:** new UC required (event_timezone на write/read + tz-aware презентація часу; minor-units контракт платіжного seam) — узгодити у spec-reconciliation (**див. TASK-101**). Дотичні наявні UC перегляду/створення події та платіжного DTO отримують адитивне (non-breaking) поле.

**Impacted BR/WF:**
- Новий BR: «кожна подія MUST мати валідний IANA `event_timezone`; час презентується в поясі події».
- Новий BR: «усі гроші на API/ledger — `amount_minor:i64` + `currency`; конверсія лише в `money.rs`».
- WF create/update event: резолв `event_timezone` з міста (override дозволено, валідація IANA).

**Dependencies:**
- **TASK-101** (spec reconciliation) — фіксація нових UC/BR ПЕРЕД кодом (rules §0/§2).
- INIT-ROADMAP-SPEC — product-first, payment last (контекст: реального PSP тут немає).
- TASK-LEDGER-1 та TASK-PROV-1 — СПОЖИВАЧІ контракту `amount_minor:i64` (ця задача його ФІКСУЄ).
- Крейт `chrono-tz` (`api/Cargo.toml`); `androidx desugar_jdk_libs` coreLibraryDesugaring (`android/app/build.gradle.kts`).
- Gazetteer (migration 0015 cities) — джерело city→country→tz.

**Risks / Open Questions:**
- Backfill не має зсунути наявні `starts_at/ends_at` (UTC незмінні — лише presentation), інакше зсув уже проданих квитків (SEC-5).
- Невідоме місто без відповідника в gazetteer → дефолт `Europe/Kyiv` (узгоджено).
- DST-межі (02:30 у ніч переводу) не мають кидати виняток на жодній платформі.
- Округлення грошей: `to_minor` має падати на надлишку знаків, а не тихо округляти (втрата грошей).

## Estimation
**Level:** MEDIUM (аналітична оцінка S) — але зачіпає 4 різні шари (Infra/API, Android, Web) + docs і різні BR/WF, що за rules §12.5/§12.6 вимагає декомпозиції на `LOW`-листя ПЕРЕД кодом.
**Justification:** Обсяг кожного шару малий і чітко відокремлений (окрема міграція, окремий seam, окремий формат-хелпер на клієнт), але межі декомпозиції явні: distinct layer (API/Android/Web), sequential phase (schema→API→UI), distinct BR (money vs timezone vs i18n). Тому статус DECOMPOSED із реальними LOW-листами.

## Sub-tasks (DECOMPOSED)
| Sub-task ID | Title | Estimation | Status | Depends On |
|---|---|---|---|---|
| TASK-102-A-schema | Міграція `0033_event_timezone.sql`: `events.event_timezone` + `cities.timezone`, backfill із gazetteer, `NOT NULL DEFAULT 'Europe/Kyiv'`; idempotent-safe | LOW | READY | TASK-101 |
| TASK-102-B-money-seam | `api/src/money.rs` (`to_minor`/`from_minor`) + `currency::exponent`; checked overflow, precision-guard, negative-guard; unit-тести на кожну gazetteer-валюту | LOW | READY | TASK-101 |
| TASK-102-C-api-tz | `api/src/cities.rs` tz-резолв; `events::create/update` проставляє/валідує `event_timezone`; `EventDto/CreateEventReq/UpdateEventReq/City` DTO-розширення; 400 `invalid_timezone` | LOW | READY | TASK-102-A |
| TASK-102-D-api-dto-money | Платіжні wire-DTO (Charge/Payout/Refund/Webhook) серіалізують `amount_minor:i64`+`currency` (не Decimal); domain лишається Decimal | LOW | READY | TASK-102-B |
| TASK-102-E-android | `coreLibraryDesugaring` on; `util/Format.kt` через `java.time`; `data/Event.kt += event_timezone`; суфікс `R.string.time_local`; CreateEvent read-only пояс + advanced override | LOW | READY | TASK-102-C |
| TASK-102-F-web | `web/lib/i18n.tsx fmtDateL(iso,lang,tz)` через `Intl.DateTimeFormat`; `web/lib/api.ts Event += event_timezone`; EventCard/EventDetailClient/me-tickets/create-event | LOW | READY | TASK-102-C |
| TASK-102-G-i18n-scaffold | `values-en/strings.xml` (нові ключі: `time_local`, `event_timezone_label`); нові Android-рядки лише через `stringResource` | LOW | READY | TASK-102-E |
| TASK-102-H-rules-spec | `docs/rules.md` guardrail (substring-заборона/start-end/stringResource); `docs/spec.md` фіксація контрактів | LOW | READY | TASK-101 |

## Execution Order
1. **TASK-101** (spec reconciliation — нові UC/BR у spec ПЕРЕД кодом) → **TASK-102-H** (rules/spec фіксація).
2. **TASK-102-A** (schema+backfill) — незворотна межа, робиться першою.
3. Паралельно: **TASK-102-B** (money seam) ‖ **TASK-102-C** (API tz на базі -A).
4. **TASK-102-D** (money-DTO) після -B.
5. **TASK-102-E** (Android) ‖ **TASK-102-F** (Web) після -C.
6. **TASK-102-G** (i18n scaffold) після -E.
7. `./scripts/ci.sh` зелений (§8.6) перед done.

## Speck Reference
docs/specks/SDD-102-irreversible-contracts.md
