# SDD-001-C — Rust quality gates

**Backlog:** TASK-001-C  **Parent:** TASK-001
**Impacted UCs:** — (інфра)

## Scope / Non-goals
**Scope:** статичні гейти Rust — `rustfmt` (max_width 120, §5.2), `clippy -D warnings`, coverage через `cargo llvm-cov`.
**Non-goals:** CI-обгортка (TASK-001-B); тести доменної логіки (TASK-001-D постачає перші тести).

## Design
- `api/rustfmt.toml` — `max_width = 120`.
- Гейт-команди (документовані в development-manual, виконуються в CI):
  `cargo fmt --check` · `cargo clippy --all-targets -- -D warnings` · `cargo llvm-cov --fail-under-lines 100` (для доменних модулів).
- `llvm-tools-preview` + `cargo-llvm-cov` ставляться в CI-стадії (TASK-001-B).

## Acceptance Criteria
- **AC-1** `cargo fmt --check` чисто.
- **AC-2** `cargo clippy --all-targets -- -D warnings` без помилок.
- **AC-3** coverage-команда визначена й запускається; на доданих модулях TASK-001-D — 100% lines (§7.1).

## Security Acceptance Criteria
- **SEC-1** clippy не глушиться `#[allow]` для обходу correctness/безпеки без обґрунтування в Speck.

## Test Matrix
| AC | Gate run |
|---|---|
| AC-1 | ✅ `cargo fmt --check` |
| AC-2 | ✅ `cargo clippy -D warnings` |
| AC-3 | ✅ `cargo llvm-cov` (на модулях D) |
| SEC-1 | ✅ review: без невиправданих `#[allow]` |
