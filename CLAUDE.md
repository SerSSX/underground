# CLAUDE.md — Underground

> **General process:** `~/.claude/rules/rules.md` (always loaded) wins over this file, except where this
> project's policy is stricter: then both bind. Project policy: [`docs/rules.md`](docs/rules.md) and the
> Process section below. The old v0.5.0 doctrine is archived at
> `~/Documents/rules-archive-20261102-pre-v3.13/repos/underground/`.

## Product

Афіша + квитковий сервіс + соцмережа для концертів/вечірок/подій (UA-ринок: UAH, Київ).
Незалежний від SignalOps. Будується за дизайном Figma "underground". Бізнес-правда — у `docs/spec.md`.

## Stack (reality)

- **Android** — Kotlin + Jetpack Compose (`android/`, compileSdk/targetSdk 35, minSdk 24, AGP 8.5, Kotlin 2.0). Dark-only.
- **API** — Rust + Axum + Tokio + SQLx/PostgreSQL (`api/`).
- **Admin** — окремий Next.js/React веб-застосунок (`admin/`) поверх `/api/admin/*`.
- **Auth** — тільки Google OAuth (ID-token → app-JWT). **Тема — завжди темна.**
- **Платіж** — Lemon Squeezy (MoR), webhook-патерн як у SignalOps. Монетизація — комісійний маркетплейс
  із **ескроу**: гроші утримуються до завершення події (`HELD→CLEARING→CLEARED→PAID_OUT`, гілки `DISPUTED`/`REFUNDED`),
  потім виплачуються організатору. Уся грошова логіка — через єдиний seam `api/src/escrow.rs`.

## Process (project policy — see `docs/rules.md`)

- **Backlog-first:** кожна задача — `docs/backlog/TASK-###-*.md` ПЕРЕД аналізом/кодом. Повний lifecycle:
  `INTAKE → REFINED → ESTIMATED → READY`. `HIGH`/`IMPOSSIBLE` **MUST be decomposed** у `LOW`-листя до коду.
- **Speck-driven:** кожна задача — `docs/specks/SDD-###-*.md` з `AC-1..n` + **`SEC-1..n`** + Test Matrix. Speck-audit перед кодом.
- **Spec is truth:** не реалізовувати поведінку, що суперечить `docs/spec.md`; зміни поведінки — спершу в spec, потім тести, потім код.
- **Code quality:** без `any`/unsafe-cast; без placeholders/TODO/dead code; ≤120 char/рядок, ≤30 рядків/функція, ≤500/тип; без resource-leak.
- **Testing:** container-first; 100% unit+integration coverage; кожна сюїта — positive + negative + **[SEC]** вектори; fail-fast стадії.
- **Security audit щозадачі:** без нових high/critical; pre-existing — повідомляти явно.
- **Forbidden:** breaking API без апруву; зміна env/secrets без апруву; хардкод портів/CORS/redirect; зміна CI без запиту;
  пропуск тестів/зниження coverage; пряма мутація БД поза міграціями.

## Required docs

`docs/spec.md`, `docs/development-manual.md`, `docs/installation-manual.md`, `docs/configuration-manual.md`,
`docs/specks/`, `docs/backlog/`. План реалізації MVP→v1: `~/.claude/plans/streamed-humming-steele.md`.

## Build / run

- API: `cd api && cargo run` (порт із `.env`/`PORT`). Тести: `cargo test` (+ coverage). Container-first — див. `docker-compose.yml`.
- Android: `cd android && ANDROID_HOME=~/Library/Android/sdk ./gradlew assembleDebug`.
- Admin: `cd admin && npm run build`.
- CI-симуляція локально перед "done": `./scripts/ci.sh`.
