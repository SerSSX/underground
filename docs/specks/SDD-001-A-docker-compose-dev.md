# SDD-001-A — docker-compose dev (Postgres + API)

**Backlog:** TASK-001-A  **Parent:** TASK-001
**Impacted UCs:** — (інфра)  **Impacted BR/WF:** —

## Scope / Non-goals
**Scope:** контейнеризований one-command dev (§8.1): `docker compose up` → Postgres + API-кістяк здорові й доступні;
порти через root `.env`; multi-stage Dockerfile (non-root, HEALTHCHECK, `.dockerignore`); волюм БД у `./.dev-data/`.
**Non-goals:** API↔БД-конект і міграції (TASK-001-D); CI (TASK-001-B); production-мінімізація образу (M6).

## Design
- `api/Dockerfile` — stage `builder` (`rust:1.96-slim-bookworm`, `cargo build --release`) → stage `runtime`
  (`debian:bookworm-slim` + `ca-certificates`,`curl`; non-root `appuser`; `HEALTHCHECK` curl `/health`; `ENTRYPOINT` бінар).
- `api/.dockerignore` — `target/`, `.env*`, `*.md`.
- `docker-compose.yml` — `db` (`postgres:16-alpine`, `pg_isready` healthcheck, волюм `./.dev-data/pg`, порт `${POSTGRES_PORT}`),
  `api` (build `./api`, порт `${API_PORT}:8080`, `DATABASE_URL` на `db`, `depends_on db: service_healthy`).
- root `.env.example`/`.env` — `API_PORT`,`POSTGRES_PORT/USER/PASSWORD/DB`,`RUST_LOG` (safe dev defaults, §11).
- `scripts/dev-up.sh` — копіює `.env` за потреби, авто-резолвить зайнятий `API_PORT` (§8.1, без запиту), запускає compose.
- `.gitignore` += `.env`, `/.dev-data/`, `/.dev-temp/` (§8.2; `.env.example` комітиться).

### §8.3 justification
Dev-runtime — `debian:bookworm-slim` (потрібні shell+curl для `HEALTHCHECK`), не distroless/alpine. Production-мінімізація
(distroless/musl + digest-pin) — окрема хардненг-задача M6. Це усвідомлене, обґрунтоване відхилення (§8.3 дозволяє з justification у Speck).

## Acceptance Criteria
- **AC-1** `docker compose build` успішний (API-образ збирається multi-stage).
- **AC-2** `docker compose up -d` → `db` healthy (`pg_isready`) і `api` healthy (`/health`).
- **AC-3** `curl http://localhost:${API_PORT}/health` → `{"status":"ok"}`; `/api/version` → name+version.
- **AC-4** Порти змінюються лише через `.env` (зміна `API_PORT` → новий проброс), без правок compose/коду.
- **AC-5** `scripts/dev-up.sh` при зайнятому `API_PORT` обирає вільний і оновлює `.env`.

## Security Acceptance Criteria
- **SEC-1** API-контейнер працює під non-root (`appuser`), не root.
- **SEC-2** `.env` не потрапляє в git і в образ (`.gitignore` + `.dockerignore`); секретів-продакшн немає (лише dev-дефолти).
- **SEC-3** БД-порт біндиться локально; пароль БД лише з env, не хардкод у compose/Dockerfile.

## Failure Modes / Error Mapping
- Зайнятий порт → `dev-up.sh` авто-резолвить (AC-5); прямий `compose up` із зайнятим портом → зрозуміла помилка bind.
- БД не готова → `api` чекає `depends_on: service_healthy`.

## Test Matrix
| AC | Unit | Integration (compose up + curl) | Curl Dev | Prod Stack |
|---|---|---|---|---|
| AC-1 | — | ✅ build | — | ⬜ |
| AC-2 | — | ✅ healthchecks | — | ⬜ |
| AC-3 | — | ✅ | ✅ | ⬜ |
| AC-4 | — | ✅ (override `API_PORT`) | ✅ | ⬜ |
| AC-5 | — | ✅ (busy-port → resolve) | — | ⬜ |
| SEC-1 | — | ✅ (`docker inspect` user) | — | ⬜ |
| SEC-2 | — | ✅ (git check-ignore / image scan) | — | ⬜ |
| SEC-3 | — | ✅ (env-only password) | — | ⬜ |

> Prod Stack (§6.5 ст.7) — після production-мінімізації (M6); тут поза скоупом.
