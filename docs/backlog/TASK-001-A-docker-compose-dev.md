# TASK-001-A — docker-compose dev (Postgres + API)

**Status:** DONE
**Created:** 2026-06-12  **Updated:** 2026-06-12
**Parent:** TASK-001  **Speck:** docs/specks/SDD-001-A-docker-compose-dev.md

## Verification (2026-06-12)
`docker compose build` ✅ · `up -d` → `db` healthy + `api` healthy ✅ · `/health`=`{"status":"ok"}`, `/api/version` ✅ ·
AC-4 `API_PORT=8099` → `:8099/health` ✅ · SEC-1 `id`=`uid=10001(appuser)` ✅ · SEC-2 `git check-ignore .env` ✅,
`.env.example` tracked ✅ · SEC-3 пароль лише з `${POSTGRES_PASSWORD}` ✅ · `compose down` чисто ✅.
AC-5 (`dev-up.sh` авто-резолв порту) — реалізовано; стрес-тест зайнятого порту — у curl-сюїті TASK-001-B.

## Raw Request
> Листя епіку INFRA з TASK-001 (рішення «строго інфра зараз»): one-command dev-середовище в контейнерах.

## Refined Description
**Scope:** `docker compose up` піднімає Postgres + API (поточний кістяк `/health`, `/api/version`), порти конфігуровані
через root `.env` (§8.1). Multi-stage Dockerfile для API (non-root + HEALTHCHECK + `.dockerignore`, §8.3/§11),
`./.dev-data/` для волюму БД (§8.2), helper `scripts/dev-up.sh` із авто-резолвом зайнятого порту.
**Non-goals:** підключення API до БД (TASK-001-D); сід (TASK-001-D); CI-скрипт (TASK-001-B); distroless/alpine-мінімізація
production-образу (хардненг-задача M6 — обґрунтовано в Speck §8.3).
**Impacted UCs:** — (інфра). **Impacted BR/WF:** —.
**Dependencies:** контейнерний рантайм (colima — встановлено).
**Risks:** перша збірка Rust-образу повільна (компіляція); pin образів — debian-slim для dev, мінімізація — пізніше.

## Estimation
**Level:** LOW
**Justification:** ≤6 інфра-файлів, без доменної логіки; чіткі AC; верифікація `compose up`→`/health`.

## Speck Reference
docs/specks/SDD-001-A-docker-compose-dev.md
