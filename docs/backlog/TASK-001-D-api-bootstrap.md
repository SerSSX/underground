# TASK-001-D — API bootstrap (config/db/error/migrations)

**Status:** DONE
**Created:** 2026-06-12  **Updated:** 2026-06-12
**Parent:** TASK-001  **Speck:** docs/specks/SDD-001-D-api-bootstrap.md

## Verification (2026-06-12)
lib+bin структура (`src/lib.rs`) для тестованості; `config/db/error/state/events/routes` + міграція `0001_init.sql`.
**11 тестів зелені:** unit config×4 + error×2; integration `db_connect` (+negative bad-url), `events_repo` (empty + published-sorted),
`http_api` (health/version/events 200), `http_api_errors` (events→500 on DB error). Lines/functions coverage 100% (див. TASK-001-C).
Container e2e: `docker compose up` → `db`+`api` healthy, `/api/events`→`[]`, у логах міграції+listening ✅.
Виправлено в ході: Dockerfile тепер копіює `migrations/` (потрібно для `migrate!`); `POSTGRES_PORT=5544` (колізія з хостовим Postgres :5432).
SEC: параметризовані запити (sqlx) ✅; помилка БД→500 без витоку (тіло `{"error":"database error"}`) ✅; креди лише з env ✅.

## Raw Request
> Листя INFRA з TASK-001: підняти SQLx-персистентність, конфіг, помилки, міграції й перший read-зріз, строго з тестами.
> (Переробка відкоченого ad-hoc M0 через backlog→speck→код.)

## Refined Description
**Scope:** `config.rs` (env), `db.rs` (PgPool + міграції на старті), `error.rs` (AppError→HTTP), `state.rs`,
міграція `0001_init.sql` (`users`,`events`), `events.rs` (`fetch_published` + `GET /api/events`). Тести: unit (config/error) +
integration (`#[sqlx::test]`, ephemeral БД §6.3) для репозиторію подій. API в контейнері конектиться до Postgres.
**Non-goals:** auth (TASK-002), CRUD/seed подій (TASK-003+), будь-яка доменна логіка поза read-списком.
**Impacted UCs:** UC-002 (частково, read). **Impacted BR/WF:** BR-009 (схема tickets_left). **Dependencies:** TASK-001-A, TASK-001-C.

## Estimation
**Level:** LOW  **Justification:** ≤8 файлів, чіткий read-зріз, передбачуваний тест-сурфейс.

## Speck Reference
docs/specks/SDD-001-D-api-bootstrap.md
