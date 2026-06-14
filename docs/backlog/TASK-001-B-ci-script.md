# TASK-001-B — CI fail-fast script

**Status:** DONE
**Created:** 2026-06-12  **Updated:** 2026-06-12
**Parent:** TASK-001  **Speck:** docs/specks/SDD-001-B-ci-script.md

## Verification (2026-06-12)
`./scripts/ci.sh` → усі стадії зелені, друкує `CI OK`: db up → fmt --check → clippy -D warnings → 11 тестів →
coverage gate `--fail-under-lines 100` (пройдено) → image build → container e2e (`/health`, `/api/events []`) → down. Fail-fast `set -euo pipefail`; креди лише з `.env`.

## Raw Request
> Листя INFRA з TASK-001: локальна CI-симуляція fail-fast (§6.5, §8.6) поверх гейтів TASK-001-A/C/D.

## Refined Description
**Scope:** `scripts/ci.sh` — стадії, що зупиняються на першій помилці: db up → fmt → clippy → tests →
coverage (`--fail-under-lines 100`) → image build → container e2e (curl) → down. Читає `.env`.
**Non-goals:** GitHub Actions YAML (поза скоупом MVP-інфра); Android/Admin стадії (додамо з TASK-001-E/F).
**Impacted UCs:** —. **Dependencies:** TASK-001-A/C/D (DONE).

## Estimation
**Level:** LOW  **Justification:** один скрипт, що оркеструє наявні гейти.

## Speck Reference
docs/specks/SDD-001-B-ci-script.md
