# CLAUDE.md — Underground

> **Governing doctrine:** [`docs/rules.md`](docs/rules.md) (v0.5.0) is **STRICT / NON-NEGOTIABLE** and authoritative.
> On any conflict, the order in its §0 applies: System → User → `docs/rules.md` → `docs/spec.md` →
> `docs/specks/*` → `docs/backlog/*` → `README.md`. If truly ambiguous — ask before acting.
> This file is the always-loaded summary; `docs/rules.md` wins over it.

## Product

Афіша + квитковий сервіс + соцмережа для концертів/вечірок/подій (UA-ринок: UAH, Київ).
Незалежний від SignalOps. Будується за дизайном Figma "underground". Бізнес-правда — у `docs/spec.md`.

## Stack (reality)

- **Android** — Kotlin + Jetpack Compose (`android/`, compileSdk/targetSdk 35, minSdk 24, AGP 8.5, Kotlin 2.0). Dark-only.
- **API** — Rust + Axum + Tokio + SQLx/PostgreSQL (`api/`).
- **Admin** — окремий Next.js/React веб-застосунок (`admin/`) поверх `/api/admin/*`.
- **Auth** — Google OAuth (ID-token → app-JWT) як основний метод **+ passwordless email one-time code** як
  задокументований виняток для claim/guest-контексту (identity-merge за email; див. `docs/spec.md` UC-001/UC-013, BR-007/016/017).
  **Тема — завжди темна.** (Оновлено TASK-101; заміняє попереднє «тільки Google OAuth».)
- **Платіж** — **product-first, payment last**: провайдер — змінний модуль (`trait PaymentProvider`, seam
  `api/src/escrow.rs`); перший реліз (Simple Launch) працює на `NoopProvider`/`TestProvider` без реальних грошей.
  Монетизація — комісійний маркетплейс із **ескроу** (`HELD→CLEARING→CLEARED→PAID_OUT`, гілки `DISPUTED`/`REFUNDED`),
  provider-neutral. Реальний PSP (**Stripe Connect + Payoneer**, US-entity via Atlas) вмикається ОСТАННІМ (Фаза 3–4).
  Уся грошова логіка — виключно через seam `api/src/escrow.rs` (BR-006, BR-021).

## Process (non-negotiable — see `docs/rules.md`)

- **Backlog-first (§12):** кожна задача — `docs/backlog/TASK-###-*.md` ПЕРЕД аналізом/кодом. Повний lifecycle:
  `INTAKE → REFINED → ESTIMATED → READY`. `HIGH`/`IMPOSSIBLE` **MUST be decomposed** у `LOW`-листя до коду (§12.5/§12.6).
- **Speck-driven (§3):** кожна задача — `docs/specks/SDD-###-*.md` з `AC-1..n` + **`SEC-1..n`** + Test Matrix. Speck-audit перед кодом.
- **Spec is truth (§2):** не реалізовувати поведінку, що суперечить `docs/spec.md`; зміни поведінки — спершу в spec, потім тести, потім код.
- **Code quality (§5):** без `any`/unsafe-cast; без placeholders/TODO/dead code; ≤120 char/рядок, ≤30 рядків/функція, ≤500/тип; без resource-leak.
- **Testing (§6–§7):** container-first; 100% unit+integration coverage; кожна сюїта — positive + negative + **[SEC]** вектори; fail-fast стадії.
- **Security audit щозадачі (§9.1):** без нових high/critical; pre-existing — повідомляти явно.
- **Forbidden (§10):** breaking API без апруву; зміна env/secrets без апруву; хардкод портів/CORS/redirect; зміна CI без запиту;
  пропуск тестів/зниження coverage; пряма мутація БД поза міграціями.

## Required docs (§2.1)

`docs/spec.md`, `docs/development-manual.md`, `docs/installation-manual.md`, `docs/configuration-manual.md`,
`docs/specks/`, `docs/backlog/`. План реалізації MVP→v1: `~/.claude/plans/streamed-humming-steele.md`.

## Build / run

- API: `cd api && cargo run` (порт із `.env`/`PORT`). Тести: `cargo test` (+ coverage). Container-first — див. `docker-compose.yml`.
- Android: `cd android && ANDROID_HOME=~/Library/Android/sdk ./gradlew assembleDebug`.
- Admin: `cd admin && npm run build`.
- CI-симуляція локально перед "done" (§8.6): `./scripts/ci.sh`.
