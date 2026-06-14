# TASK-001-C — Rust quality gates

**Status:** DONE
**Created:** 2026-06-12  **Updated:** 2026-06-12
**Parent:** TASK-001  **Speck:** docs/specks/SDD-001-C-rust-gates.md

## Verification (2026-06-12)
`rustfmt.toml` (max_width 120) ✅ · `cargo fmt --check` чисто ✅ · `cargo clippy --all-targets -- -D warnings` без зауважень ✅ ·
`cargo-llvm-cov` 0.8.7 встановлено; на доданих модулях (config/db/error/events/routes) **lines 100%, functions 100%**,
regions 99.21% (1 непокрита — захисна гілка помилки `sqlx::migrate!?` у `db.rs`, недосяжна без зумисне зламаної міграції; свідомий gap, §9.1). `main.rs` виключено з coverage (bootstrap).

## Raw Request
> Листя INFRA з TASK-001: гейти якості Rust (§5,§7.1) — clippy/fmt/coverage.

## Refined Description
**Scope:** `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo llvm-cov` як coverage-інструмент; `rustfmt.toml`
(ліміт 120, §5.2). Гейти інтегруються у CI (TASK-001-B).
**Non-goals:** сам CI-скрипт (TASK-001-B); доменний код (TASK-001-D).
**Impacted UCs:** —. **Dependencies:** —.

## Estimation
**Level:** LOW  **Justification:** конфіг + перевірка, без логіки.

## Speck Reference
docs/specks/SDD-001-C-rust-gates.md
