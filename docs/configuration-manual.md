# Configuration Manual — Underground

> § numbers in this document refer to the archived v0.5.0 doctrine at
> `~/Documents/rules-archive-20261102-pre-v3.13/repos/underground/rules.md` (history only). Process today:
> `~/.claude/rules/rules.md`; project policy: `docs/rules.md`.


Уся конфігурація — через env (§8.1, §10). Жодних хардкодів портів/CORS/redirect/base-URL.

## API env (`api/.env`, зразок — `api/.env.example`)

| Змінна | Дефолт (dev) | Призначення |
|---|---|---|
| `DATABASE_URL` | `postgres://localhost/underground_dev` | Підключення до Postgres |
| `PORT` | `8080` | Порт API (конфігуровний, §8.1) |
| `RUST_LOG` | `info` | Рівень логів |
| `JWT_SECRET` | — (required у не-dev) | Підпис app-JWT |
| `GOOGLE_CLIENT_ID` | — | Верифікація Google ID-token (aud) |
| `LEMONSQUEEZY_API_KEY` | — | LS API (checkout, refunds) |
| `LEMONSQUEEZY_STORE_ID` | — | LS store |
| `LEMONSQUEEZY_WEBHOOK_SECRET` | — | Перевірка HMAC-SHA256 підпису webhook |
| `LEMONSQUEEZY_VARIANT_PLAN_MAP` | `{}` | JSON-мапа варіант→подія/ціна |
| `PLATFORM_COMMISSION_BPS` | `1000` | Комісія платформи (bps, 1000 = 10%) — BR-003 |
| `ESCROW_CLEARING_DAYS` | `5` | Clearing-вікно після `ends_at` — BR-001 |
| `DISPUTE_THRESHOLD_BPS` | `2000` | Поріг скарг власників для авто-`DISPUTED` — BR-002 |
| `AUTH_LOGIN_RATE_LIMIT` | `10/min` | Ліміт спроб логіну на email/IP (brute-force захист) — BR-017 |
| `ADMIN_EMAILS` | — | Кома-список email із роллю ADMIN |

> Паролі — argon2id (параметри-константи в коді; ніколи не в env/логах) — BR-016.

> Кожна змінна валідовується на старті; відсутність required у проді = fail-fast. Зміна env/secrets — лише з апрувом (§10).

## Ports (§8.1, hard)

- Експонувати лише потрібні порти; усі — через `.env`; `docker-compose.yml` бере порти зі змінних.
- Конфлікт порту → авто-резолв (detect → вільний порт → оновити `.env` → retry), без запиту до користувача.

## Dev data / temp (§8.2)

- `./.dev-data/` — dev-волюми (видалити, щоб скинути систему). Тести на нього не покладаються (§6.3).
- `./.dev-temp/` — тимчасові build-артефакти (безпечно видаляти).

## Test device matrix (§7.3) — для `admin/` (Playwright projects)

- **Desktop**, **Tablet** (portrait + landscape).
- **Mobile** (portrait + landscape): останні 3 покоління iPhone, Pixel, Samsung-флагманів (усі моделі поколінь).
- Реалізація — Playwright **projects**; кожен тест бене по всіх проєктах, окрім явно зскоупленого (з апрувом).
- Відео Full HD (1920×1080), курсор видимий, human-like швидкість (§7.4).

> Android-клієнт — нативний; UI-паритет покривається Compose-preview/instrumented-тестами на емуляторах
> (матриця API-рівнів/розмірів екрана визначається в Speck відповідної UI-задачі), не Playwright.

## Performance / size budgets (§9.3)

- API: p95 латентність read-ендпоінтів — задається в Speck відповідної задачі; без avoidable-регресій.
- Android APK debug-розмір — фіксувати baseline у першій UI-задачі; стежити за приростом.
- Production-образи API — non-root + `HEALTHCHECK` + `.dockerignore` (§8.3); пріоритет distroless/alpine.
