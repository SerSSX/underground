# SDD-001-D — API bootstrap (config/db/error/migrations)

**Backlog:** TASK-001-D  **Parent:** TASK-001
**Impacted UCs:** UC-002 (read-частина)  **Impacted BR/WF:** BR-009

## Scope / Non-goals
**Scope:** персистентність і перший тестований read-зріз: `config`, `db`(PgPool+міграції), `error`, `state`, `events`
(`fetch_published` + `GET /api/events`), міграція `users`+`events`. Container-first; ephemeral test-БД (§6.3).
**Non-goals:** auth, CRUD/seed, доменна логіка поза read-списком.

## Design
- `config.rs` — `Config::from_env()` (`DATABASE_URL`,`PORT`; dev-дефолти).
- `db.rs` — `connect()` → `PgPoolOptions` + `sqlx::migrate!()`.
- `error.rs` — `AppError::Db` → 500 (лог), розширюється згодом.
- `events.rs` — `fetch_published(&PgPool)` (лише `status='published'`, сорт `starts_at`), хендлер обгортає у JSON.
- `migrations/0001_init.sql` — `users`,`events` (+ індекси), `tickets_left>=0` гарантується доменно (BR-009).

## Acceptance Criteria
- **AC-1** `cargo build`/clippy/fmt чисто; контейнер `api` стартує, застосовує міграції, healthy.
- **AC-2** `GET /api/events` на чистій БД → `200 []`.
- **AC-3** `fetch_published` повертає лише `published`, відсортовані за `starts_at` (integration, ephemeral БД).
- **AC-4** unit: `Config::from_env` дефолти; `AppError::Db`→500.

## Security Acceptance Criteria
- **SEC-1** Усі запити — параметризовані (sqlx), без конкатенації SQL (анти-інʼєкція).
- **SEC-2** Помилка БД → 500 без витоку внутрішніх деталей у тіло відповіді (деталі лише в лог).
- **SEC-3** Конект-рядок/креди — лише з env (не в коді/логах).

## Failure Modes / Error Mapping
- БД недоступна на старті → fail-fast (контейнер не healthy).
- Помилка запиту → `AppError::Db` → 500 `{"error":"database error"}`.

## Test Matrix
| AC | Unit | Integration (sqlx::test) | Curl Dev |
|---|---|---|---|
| AC-1 | — | ✅ (compose up healthy) | ✅ |
| AC-2 | — | ✅ | ✅ |
| AC-3 | — | ✅ (published-only + order) | — |
| AC-4 | ✅ | — | — |
| SEC-1 | — | ✅ (параметризовані) | — |
| SEC-2 | ✅ (error body) | — | ✅ |
| SEC-3 | ✅ (config) | — | — |
