# SDD-103 — CI/CD pipeline + container-first тест-харнес

**Backlog:** TASK-103 (DECOMPOSED: A–G)
**Impacted UCs:** — (інфраструктура; поведінки продукту не змінює; нових UC немає → TASK-101 spec-reconciliation не потрібна)
**Impacted BR/WF:** — прямих немає; робить §6.3/§6.5/§7.3/§8.6 executable-гейтами. Provider-neutral: фінансові шляхи — під `NoopProvider` (`api/src/escrow.rs`).

## Scope / Non-goals
**Scope:**
- Єдиний fail-fast конвеєр у двох рантаймах: локальний `scripts/ci.sh` (+ `scripts/ci-lib.sh`) і хмарний `.github/workflows/ci.yml`, обидва зі стадіями §6.5 у порядку `typecheck/lint → unit → integration → curl(dev) → Base UI → UI → curl(prod) → prod-stack`, плюс фінальна `security/deps`.
- Ephemeral Postgres (§6.3): нова БД щоразу, без persisted volume, readiness-gate `pg_isready` з bounded retries, міграції/сіди після readiness, order-independent.
- Playwright projects каркас під device-matrix §7.3 (Desktop, Tablet portrait+landscape, Mobile portrait+landscape — останні iPhone/Pixel/Samsung), зі smoke-специфікаціями.
- Coverage-gate §7.1 збережено (`cargo llvm-cov --fail-under-lines 100`).
- Модулі: `scripts/ci.sh`, `scripts/ci-lib.sh`, `.github/workflows/ci.yml`, `docker-compose.yml`, `playwright.config.ts` (web+admin), `admin/app/**`, `web/app/**`, `android/app`, гейти над `api/src/*.rs` (включно з `escrow.rs` seam).

**Non-goals:** реальні e2e-UC-тести (лише каркас+smoke); реальний PSP/webhook; deploy/CD у прод; Android emulator-matrix у хмарі; зміна coverage-політики.

## Acceptance Criteria
- **AC-1:** `./scripts/ci.sh` на зеленому дереві проходить усі стадії §6.5 у заданому порядку й друкує `CI OK` ненульовим кодом лише при успіху.
- **AC-2:** Fail-fast — падіння будь-якої стадії негайно зупиняє конвеєр ненульовим exit-кодом; наступні стадії не виконуються (`set -euo pipefail` + `needs:` у GH).
- **AC-3:** Ephemeral Postgres — кожен запуск стартує з порожньої БД (без persisted volume); readiness-gate `pg_isready` з bounded retries; міграції/сіди застосовуються лише після readiness; тести проходять при повторному запуску без ручного очищення (order-independent, §6.3).
- **AC-4:** Стадія `typecheck/lint` фейлить на Rust (`cargo fmt --check`, `clippy -D warnings`), TS (`tsc --noEmit`, `eslint`) для `admin/`+`web/`, і Android (`./gradlew lint`); будь-яке порушення → нульовий не-успіх.
- **AC-5:** Coverage-стадія фейлить при lines < 100% (`--fail-under-lines 100`), зберігаючи §7.1.
- **AC-6:** Curl-стадії покривають публічні маршрути на dev-образі (стадія 3) і на prod-образі (стадія 6), перевіряючи status + shape (`/health`, `/api/events`), і проганяються знову проти prod-stack (стадія 7).
- **AC-7:** `playwright.config.ts` визначає projects для всіх класів device-matrix §7.3 (Desktop; Tablet portrait+landscape; Mobile portrait+landscape — останні iPhone/Pixel/Samsung); Base UI-стадія запускає smoke-spec headless і фейлить при недоступному застосунку.
- **AC-8:** `.github/workflows/ci.yml` віддзеркалює той самий 7-стадійний ланцюг із ephemeral Postgres як service-container і fail-fast через `needs:`; жоден крок не хардкодить порти/креди/CORS/redirect — усе з `.env`/GH-секретів (§8, §10).
- **AC-9:** Фінальна `security/deps` стадія запускає `cargo audit`/`cargo deny` + `npm audit`; нові high/critical фейлять конвеєр; pre-existing — у явному allowlist із записом (§9.1).
- **AC-10:** Жодна стадія не піднімає реальний PSP; будь-який фінансовий шлях у тестах іде через `NoopProvider`/Test seam (`api/src/escrow.rs`) — конвеєр зелений без реальних платіжних кредів (provider-neutral, §8).

## Security Acceptance Criteria
- **SEC-1 (secret-leak):** БД-креди/секрети надходять лише з `.env` (лок.) або GH-секретів (хмара); жодного хардкоду в `ci.sh`/`ci.yml`/`docker-compose.yml`; логи стадій не друкують значення секретів (маскування).
- **SEC-2 (dependency-supply-chain):** `security/deps` блокує будь-яку нову high/critical vuln у Rust/npm-залежностях; allowlist pre-existing веде явний, датований запис — новий advisory поза allowlist фейлить (§9.1).
- **SEC-3 (injection у curl-векторах):** curl-стадії включають негативні/adversarial вектори (SQLi-подібний payload у query, oversized body, malformed JSON) проти публічних маршрутів — сервіс відповідає коректним 4xx, не 5xx і без ехо-leak (§6.4).
- **SEC-4 (authz/IDOR у curl-векторах):** curl-стадії перевіряють, що захищені маршрути без/з чужим токеном → 401/403, не 200 (§6.4 authz/IDOR).
- **SEC-5 (no-hardcoded-ports/CORS/redirect):** статична перевірка (grep-gate у `ci.sh`), що конвеєр і compose не містять хардкод-портів/CORS-origin/redirect поза `.env` (§10 forbidden).
- **SEC-6 (ephemeral-isolation):** ephemeral Postgres не переживає run і не монтує persisted volume — жодних залишкових даних/кред між прогонами (§6.3); перевіряється відсутністю `volumes:` персистенції для CI-БД.

## Failure Modes / Error Mapping
| Режим | Симптом | Очікувана поведінка |
|---|---|---|
| Readiness timeout БД | `pg_isready` не проходить за bounded retries | стадія `db up` фейлить ненульово, конвеєр стоп (AC-2/AC-3) |
| Lint/typecheck порушення | fmt/clippy/tsc/eslint/gradle-lint red | стадія 1 фейлить, наступні не стартують (AC-4/AC-2) |
| Coverage < 100% | llvm-cov `--fail-under-lines` | стадія coverage ненульова (AC-5) |
| Publ. маршрут 5xx/невірний shape | curl негативний | стадія curl фейлить (AC-6/SEC-3) |
| Playwright: застосунок недоступний | Base UI smoke падає | стадія Base UI фейлить (AC-7) |
| Нова high/critical vuln | audit/deny/npm audit | `security/deps` фейлить (AC-9/SEC-2) |
| Хардкод порту/секрету | grep-gate/маскування | ci.sh фейлить до запуску стадій (SEC-1/SEC-5) |
| Реальний PSP-крок у тесті | немає Test/Noop seam | конфіг помилка → фейл; шлях має йти через NoopProvider (AC-10) |
| Persisted volume у CI-БД | залишкові дані між run | compose-перевірка фейлить (SEC-6) |

## Test Matrix
| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 (усі стадії→CI OK) | ⬜ | ⬜ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-2 (fail-fast) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-3 (ephemeral DB §6.3) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-4 (typecheck/lint усі шари) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-5 (coverage 100%) | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-6 (curl route coverage) | ⬜ | ⬜ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-7 (Playwright device-matrix каркас §7.3) | ⬜ | ⬜ | ⬜ | ✅ | ✅ | ⬜ | ✅ |
| AC-8 (GH Actions дзеркало) | ⬜ | ⬜ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-9 (security/deps) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| AC-10 (provider-neutral NoopProvider) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-1 (secret-leak / env-only) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-2 (dependency supply-chain) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-3 (injection curl-вектори) | ⬜ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-4 (authz/IDOR curl-вектори) | ⬜ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-5 (no-hardcoded ports/CORS/redirect) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-6 (ephemeral isolation) | ⬜ | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
