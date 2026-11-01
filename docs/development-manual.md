# Development Manual — Underground

> § numbers in this document refer to the archived v0.5.0 doctrine at
> `~/Documents/rules-archive-20261102-pre-v3.13/repos/underground/rules.md` (history only). Process today:
> `~/.claude/rules/rules.md`; project policy: `docs/rules.md`.


Як вестись розробка: загальний процес — `~/.claude/rules/rules.md`, політика проєкту — `docs/rules.md`. Цей документ — операційний.

## Repository layout

```
underground/
├── api/                 # Rust + Axum + SQLx (PostgreSQL)
├── android/             # Kotlin + Jetpack Compose (dark-only)
├── admin/               # Next.js web-admin (поверх /api/admin/*) — створюється інфра-задачею
├── docs/                # rules, spec, manuals, backlog/, specks/
├── docker-compose.yml   # one-command dev (§8.1) — створюється інфра-задачею
└── scripts/ci.sh        # CI-симуляція (§8.6) — створюється інфра-задачею
```

## Workflow для будь-якої задачі (обов'язково)

1. **Backlog (§12):** створити `docs/backlog/TASK-###-*.md` ДО аналізу/коду. Intake (Raw Request verbatim) → Refinement
   (крос-перевірка зі spec + іншими тасками) → Estimation. `HIGH`/`IMPOSSIBLE` → декомпозиція до `LOW`-листя (§12.6).
2. **Speck (§3):** `docs/specks/SDD-###-*.md` — scope/non-goals, `AC-1..n`, `SEC-1..n`, failure modes, Test Matrix
   (кожен AC/SEC → стадії тестів), impacted UC/BR/WF. Speck-audit проти `docs/spec.md` перед кодом.
3. **Spec (§2):** якщо змінюється поведінка — спершу оновити `docs/spec.md` (і baselines, якщо UI), потім тести, потім код.
4. **Implement (§5):** повний робочий код, без placeholders/dead-code; ліміти §5.2; без resource-leak.
5. **Test (§6–§7):** додати unit/integration/curl/UI з positive + negative + **[SEC]** векторами; 100% coverage; нічого не пропускати.
6. **Security audit (§9.1):** без нових high/critical; pre-existing — повідомити явно.
7. **Gates:** прогнати fail-fast стадії (§6.5) і CI-симуляцію (§8.6) ДО «done». Оновити статус таска → `DONE`.

## Quality gates (мають бути зелені перед «done»)

| Стадія | Команда (ціль) |
|---|---|
| Rust build/lint | `cargo build` (без warnings), `cargo clippy -- -D warnings`, `cargo fmt --check` |
| Rust тести + coverage | `cargo test`; покриття через `cargo llvm-cov` (100% lines+branches+functions, §7.1) |
| Android build | `ANDROID_HOME=~/Library/Android/sdk ./gradlew assembleDebug` |
| Android тести | `./gradlew test` (unit) + `connectedAndroidTest`/Compose UI (інструментовані) |
| Admin | `npm run build`, `npm test` (+ Playwright device-matrix §7.3) |
| Curl (API) | `scripts/curl-tests.sh` проти dev/prod образів (§7.2) |
| CI-симуляція | `./scripts/ci.sh` (§8.6) |

> Стек = Rust API + нативний Android (+ web-admin). Web-only пункти правил (Playwright §7.3, Angular §8.4)
> застосовуються до `admin/`; для Android UI-паритет покривається Compose/instrumented-тестами. Інфра
> (Docker, CI, coverage-гейти) ставиться інфра-задачами ПЕРЕД фіч-кодом (рішення: «строго інфра зараз»).

## Conventions

- Гроші — лише через `api/src/escrow.rs` (BR-006). Карти не зберігаємо (BR-004).
- Тести не видаляти/не змінювати без явного апруву (§6.1). Додавати — завжди можна.
- Без хардкоду портів/CORS/redirect URI — усе через env (§10). Порти конфігуровані через `.env` (§8.1).
- Гілки: фіча-гілка від `main`; коміт/пуш — лише на запит користувача.
