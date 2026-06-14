# SDD-001-B — CI fail-fast script

**Backlog:** TASK-001-B  **Parent:** TASK-001
**Impacted UCs:** — (інфра)

## Scope / Non-goals
**Scope:** `scripts/ci.sh` — container-first fail-fast симуляція CI (§6.5, §8.6): зупинка на першій помилці.
**Non-goals:** хмарний CI YAML; Android/Admin стадії (додаються разом із TASK-001-E/F).

## Design
Стадії (по порядку, `set -euo pipefail`): `db up`(+readiness) → `cargo fmt --check` → `cargo clippy --all-targets -- -D warnings`
→ `cargo test` (DATABASE_URL з `.env`) → `cargo llvm-cov --fail-under-lines 100` → `docker compose build api`
→ `docker compose up -d` + curl `/health`,`/api/events` → `docker compose down`.

## Acceptance Criteria
- **AC-1** `./scripts/ci.sh` проходить усі стадії на зеленому дереві й друкує `CI OK`.
- **AC-2** Падіння будь-якої стадії зупиняє скрипт ненульовим кодом (fail-fast).
- **AC-3** Coverage-стадія завалюється, якщо lines < 100% (`--fail-under-lines 100`).

## Security Acceptance Criteria
- **SEC-1** Креди БД у скрипті лише з `.env` (не хардкод).

## Test Matrix
| AC | Запуск |
|---|---|
| AC-1 | ✅ `./scripts/ci.sh` → CI OK |
| AC-2 | ✅ (демонстрація fail-fast — за потреби) |
| AC-3 | ✅ `--fail-under-lines 100` активний |
| SEC-1 | ✅ env-only |
