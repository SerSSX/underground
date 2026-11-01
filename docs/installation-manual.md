# Installation Manual — Underground

> § numbers in this document refer to the archived v0.5.0 doctrine at
> `~/Documents/rules-archive-20261102-pre-v3.13/repos/underground/rules.md` (history only). Process today:
> `~/.claude/rules/rules.md`; project policy: `docs/rules.md`.


## Prerequisites

| Інструмент | Версія | Призначення |
|---|---|---|
| Rust (rustup) | stable (≥1.96) | `api/` |
| PostgreSQL | 14+ | БД (локально або через Docker) |
| JDK | 17 | Android build |
| Android SDK | platform android-35 | `android/` (`ANDROID_HOME=~/Library/Android/sdk`) |
| Node.js | LTS (18+) | `admin/` |
| Docker + Compose | latest | container-first тести й one-command dev (§8.1) |

## One-command dev (ціль §8.1)

```bash
docker compose up --build      # піднімає API + PostgreSQL (+ admin), seeded & ready
```

Порти конфігуровані через `.env` (§8.1, §10) — без хардкоду. `docker-compose.yml` і seed створюються інфра-задачею
(див. `docs/backlog/TASK-001-*`); до того — локальний запуск нижче.

## Local run (без Docker, до інфра-задачі)

```bash
# API
cd api
cp .env.example .env           # виставити DATABASE_URL, PORT
cargo run                      # http://localhost:${PORT:-8080}

# Android
cd android
echo "sdk.dir=$HOME/Library/Android/sdk" > local.properties
ANDROID_HOME=~/Library/Android/sdk ./gradlew assembleDebug

# Admin
cd admin
npm install && npm run dev
```

## База даних

- Dev-БД піднімається через `docker-compose.yml` (ephemeral, §6.3 для тестів) або локальний Postgres.
- Схема — лише через міграції SQLx (`api/migrations/*.sql`); пряма мутація БД заборонена (§10).
- Скидання локального стану — видалити `./.dev-data/` (§8.2).

## Зовнішні сервіси (ключі — у `.env`, не комітити)

- **Google OAuth** — `GOOGLE_CLIENT_ID` (верифікація ID-token).
- **Lemon Squeezy** — `LEMONSQUEEZY_API_KEY` / `_STORE_ID` / `_WEBHOOK_SECRET` / `_VARIANT_PLAN_MAP`.

Деталі змінних — у `docs/configuration-manual.md`.
