# Configuration Manual — Underground

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
| `GEOCODER_URL` | `https://photon.komoot.io` | Геокодер OpenStreetMap (Photon) для `/api/geo/*` (TASK-303); лише http(s), без логіна/query |
| `GEO_LIMIT_PER_MIN` | `60` | Запити до `/api/geo/search`, `/reverse`, `/timezone` за хвилину з одного IP (IPv6 — з однієї /64) |
| `GEO_MISS_LIMIT_PER_MIN` | `20` | Із них звернень до геокодера (промахів кешу) за хвилину з одного IP |
| `GEO_UPSTREAM_LIMIT_PER_MIN` | `120` | Усього звернень до геокодера за хвилину (fair use Photon); понад — 503 |
| `CITY_LEARN_PER_DAY` | `20` | Скільки нових міст один користувач може додати в довідник за добу |
| `PUBLIC_BASE_URL` | `http://localhost:8080` | Публічна адреса API: посилання на завантаження; у `User-Agent` до геокодера |
| `WEB_APP_URL` | `http://localhost:3001` | Адреса веба для посилань у листах (скидання пароля) |
| `RESEND_API_KEY` | — (порожньо = пошта вимкнена) | Ключ Resend для листів акаунта: посилання скидання пароля, коди входу (TASK-304). Секрет: вводить лише власник (`deploy/set-mail-key.sh`) |
| `EMAIL_FROM` | — | Відправник, напр. `Underground <no-reply@ssvstudio.dev>`; пошта увімкнена лише коли задані обидва |
| `RESEND_URL` | `https://api.resend.com` | База API Resend; лише https або http на 127.0.0.1 (тести) |
| `MAIL_DAILY_BUDGET` | `80` | Скільки листів акаунта (скидання, коди) API надсилає за будь-які 24 год (Resend Free — 100/день); вичерпано → 503 «пошта тимчасово недоступна». Окремо — не більше 5 листів на одну адресу за будь-яку годину (лічильник у таблиці `mail_sends`, адреса як SHA-256) |
| `MAIL_DAILY_PER_IP` | `10` | Скільки листів акаунта може спричинити одна IP-адреса клієнта за 24 год; понад — 429 (IP зберігається як SHA-256) |
| `IMAGE_DECODE_MEM_MB` | `192` | Межа адресного простору (МіБ) дочірнього процесу, що декодує завантажені картинки (`underground-api decode-image`); Linux її жорстко застосовує, macOS — ні. Плюс 20 с процесора і 30 с часу; перевищення → 422 «зображення надто складне» |
| `IMAGE_DECODE_CPU_SECS` / `IMAGE_DECODE_WALL_SECS` | `20` / `30` | Процесорний і загальний час дочірнього декодера (у проді не змінювати; Linux-тест ліміту піднімає їх собі до 300, бо debug-збірка повільна) |
| `UPLOAD_QUEUE` | `3` | Скільки завантажень картинок API приймає одночасно (одне декодується, решта чекають із тілом у пам’яті); понад — 503 «забагато завантажень одночасно» |
| `UPLOAD_BODY_SECS` | `60` | За скільки секунд клієнт має передати все тіло завантаження; повільніше → 408 «завантаження триває надто довго», місце в черзі звільняється. Окремо: один користувач — щонайбільше одне місце в черзі (друге одночасне завантаження → 429) |
| `IMAGE_DECODER_BIN` | власний виконуваний файл API | Шлях до програми-декодера (лише для тестів) |
| `FORGOT_LIMIT_PER_IP` / `FORGOT_LIMIT_PER_EMAIL` | `20` / `5` за годину | Запити «забули пароль» з одного IP / на одну адресу |
| `RESET_LIMIT_PER_IP` | `30` за годину | Спроби скинути пароль за посиланням з одного IP |
| `PWCHANGE_LIMIT_PER_USER` | `10` за годину | Спроби змінити пароль одним акаунтом |
| `UPLOAD_LIMIT_PER_HOUR` | `120` | Завантаження картинок одним користувачем за годину |
| `TELEMETRY_LIMIT_PER_IP` | `120` за хвилину | Події `POST /api/telemetry` з одного IP; понад — 429 |
| `TELEMETRY_DAILY_MAX` | `40000` | Скільки подій телеметрії зберігається за добу (UTC) усього, з увійденими користувачами; понад — відповідь та сама, але нічого не зберігається. Бюджет: 40 000 × до ~2,3 КБ × 30 днів ≈ 2,8 ГБ у найгіршому разі (бокс спільний із msgview) |
| `TELEMETRY_DAILY_ANON` | `20000` | Із них — від анонімних клієнтів за добу (UTC); понад — відповідь та сама, але нічого не зберігається |
| `TELEMETRY_RETENTION_DAYS` | `30` | Скільки днів зберігаються події телеметрії; старіші видаляються щогодини порціями |

> Паролі — argon2id (параметри-константи в коді; ніколи не в env/логах) — BR-016.

> Кожна змінна валідовується на старті; відсутність required у проді = fail-fast. Зміна env/secrets — лише з апрувом (§10).

> **DEV compose:** `UPLOAD_QUEUE`, `UPLOAD_BODY_SECS`, `IMAGE_DECODE_CPU_SECS` and `IMAGE_DECODE_WALL_SECS` are NOT
> passed by `deploy/docker-compose.dev.yml`, so on DEV they stay at their defaults (3, 60, 20, 30) unless the compose
> file is edited to pass them. (`IMAGE_DECODE_MEM_MB`, `MAIL_DAILY_BUDGET`, `MAIL_DAILY_PER_IP` are passed.)

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
