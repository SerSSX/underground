# TASK-103 — CI/CD pipeline + container-first тест-харнес

**Status:** DECOMPOSED
**Created:** 2026-06-29  **Updated:** 2026-06-29

## Raw Request
> INIT-CI, Phase 0. Потрібен єдиний fail-fast CI/CD-конвеєр: `scripts/ci.sh` + `.github/workflows` зі стадіями typecheck/lint → unit → integration → curl → UI → prod → security/deps. Dockerized ephemeral Postgres (нова БД щоразу, без persisted volume). Каркас Playwright projects під device matrix (§7.3). Все зупиняється на першій помилці. Оцінка орієнтовно MEDIUM. Декомпозуй на LOW-листя.

## Refined Description
**Scope:**
- Розширити наявний `scripts/ci.sh` (зараз лише API-стадії) до повних 7 fail-fast-стадій §6.5 поверх усіх поверхонь: `typecheck/lint (Rust fmt+clippy, admin/web tsc+eslint, Android lint) → unit → integration → curl(dev) → Base UI + UI (Playwright) → curl(prod-image) → prod-stack`, з фінальною стадією `security/deps` (`cargo audit/deny`, `npm audit`).
- Dockerized ephemeral Postgres як §6.3: чиста БД щоразу, без persisted volume, readiness-gate (`pg_isready`) з bounded retries, міграції/сіди лише після readiness, order-independent.
- `.github/workflows/ci.yml` — той самий 7-стадійний ланцюг у GitHub Actions, fail-fast (`needs:` ланцюг), матрична БД як ephemeral service-container; жоден крок не хардкодить порти/креди (усе з `.env`/секретів GH).
- Каркас Playwright projects під device matrix (§7.3): один `playwright.config.ts` з projects Desktop / Tablet(portrait+landscape) / Mobile(portrait+landscape для останніх iPhone/Pixel/Samsung), поки зі smoke-специфікаціями (Base UI). Наповнення реальними UC-тестами — за фіче-задачами.
- Provider-neutral: жодна стадія не піднімає реальний PSP; фінансові шляхи ганяються під `NoopProvider`/Test seam (`api/src/escrow.rs`).

**Non-goals:**
- Реальні e2e-UC-тести (їх пишуть фіче-задачі; тут лише каркас projects + smoke).
- Реальний PSP/webhook у CI (Phase 4).
- Deploy/CD у прод-середовище (release/publish) — тільки build + prod-stack smoke; справжній deploy — окрема задача поза Phase 0.
- Android instrumented/emulator-тести у хмарному CI (лише `lint` + `assembleDebug` + unit; emulator-matrix — пізніше).
- Coverage-політика вже задана §7.1 (`--fail-under-lines 100`) — не змінюємо, лише зберігаємо gate.

**Impacted UCs:** — (інфраструктура; поведінки продукту не змінює). Нових UC не вводить → spec-reconciliation TASK-101 не потрібна.
**Impacted BR/WF:** — прямих BR/WF немає; опосередковано забезпечує §6.3/§6.5/§7.3/§8.6 як executable-гейти.
**Dependencies:** TASK-001-A (docker-compose dev, DONE), TASK-001-B (ci.sh база, DONE), TASK-001-C (Rust-гейти, DONE), TASK-001-E (Android skeleton), TASK-001-F (Admin), TASK-016 (public web client — джерело web Playwright-таргета). Provider seam з TASK-008 (payments test-mode).
**Risks / Open Questions:**
- R1: Повна device-matrix §7.3 у хмарі повільна/дорога → у GH ганяємо headless Chromium-підмножину, повну matrix лишаємо для локального `ci.sh` (задокументувати відсічку).
- R2: `web/` каталогу Playwright ще немає (лише `admin/`, `android/`) → каркас створюється як частина цієї задачі, наповнення — за TASK-016.
- R3: `cargo audit` може дати pre-existing advisories → політика §9.1: фейлити лише на new high/critical, pre-existing — allowlist із явним записом.
- Q1: Чи включати Android `assembleDebug` у GH-стадію prod (час збірки) — за замовчуванням так, з кешем Gradle; підтвердити.

## Estimation
**Level:** MEDIUM → трактується як HIGH за §12.5 (перетинає distinct layers: Infra-скрипт, GH-Actions, Rust, Web, Admin, Android + sequential phases lint→unit→integration→ui→prod). За §12.6 **MUST decompose** у LOW-листя перед кодом.
**Justification:** Кожна поверхня має власний тулчейн і власні failure-modes; змішувати в один PR порушує §5/§12.6. Розбивка по шарах дає LOW-листя з чіткими межами й Depends On.

## Sub-tasks (DECOMPOSED)
| Sub-task ID | Title | Estimation | Status | Depends On |
|---|---|---|---|---|
| TASK-103-A | Ephemeral Postgres harness: чиста БД/run, readiness-gate, bounded retries, no persisted volume, migrate-after-ready, order-independent (§6.3); винести у `scripts/ci-lib.sh` | LOW | READY | TASK-001-A |
| TASK-103-B | `scripts/ci.sh` — повний 7-стадійний fail-fast ланцюг §6.5 (typecheck/lint→unit→integration→curl-dev→Base UI+UI→curl-prod→prod-stack) + фінальний security/deps; env-only креди | LOW | READY | TASK-103-A |
| TASK-103-C | Web/Admin frontend-гейти в ланцюгу: `tsc --noEmit` + `eslint` + build для `admin/` та `web/` як окремі lint/unit-кроки | LOW | READY | TASK-103-B |
| TASK-103-D | Android-гейти: `./gradlew lint` + unit `test` + `assembleDebug` як стадійні кроки (без emulator) | LOW | READY | TASK-103-B |
| TASK-103-E | Playwright projects каркас під device-matrix §7.3: `playwright.config.ts` (Desktop/Tablet×2/Mobile×2, останні iPhone/Pixel/Samsung) + smoke-spec (Base UI) для web+admin | LOW | READY | TASK-103-C |
| TASK-103-F | Security/deps стадія: `cargo audit`/`cargo deny` + `npm audit` з allowlist для pre-existing (§9.1) — new high/critical фейлять | LOW | READY | TASK-103-B |
| TASK-103-G | `.github/workflows/ci.yml`: 7-стадійний ланцюг у GH Actions (needs-fail-fast), ephemeral Postgres як service-container, секрети замість хардкоду портів/кред, кеш cargo/gradle/npm | LOW | READY | TASK-103-B, TASK-103-C, TASK-103-D, TASK-103-E, TASK-103-F |

## Execution Order
A (ephemeral harness) → B (ci.sh кістяк ланцюга) → {C (web/admin), D (android), E (playwright), F (security/deps)} паралельно → G (GH Actions зшиває все). Жодного продуктового коду; порядок = sequential-phases §12.6 (harness → orchestration → per-layer gates → cloud CI).

## Speck Reference
docs/specks/SDD-103-ci-cd-pipeline.md
