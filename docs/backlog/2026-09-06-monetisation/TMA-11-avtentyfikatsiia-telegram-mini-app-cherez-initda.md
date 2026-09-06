# TMA-11 — Автентифікація Telegram Mini App через initData — новий чутливий шлях входу

| Поле | Значення |
|---|---|
| **Фаза** | Фаза 5 — Telegram Mini App |
| **Estimation** | L |
| **Status** | `INTAKE` |
| **Критичний шлях до першого платежу** | ні |
| **Depends on** | — |

## Навіщо

У репозиторії немає жодного рядка Telegram: ні валідації initData, ні `users.telegram_id`. Це буде ТРЕТІЙ спосіб отримати наш JWT поруч із паролем і поштовим кодом, тобто новий вхід у систему — і найчастіше зламуваний, бо його реалізують «як у прикладі з блогу»: беруть `initDataUnsafe`, довіряють полю `user` і на цьому все. Задача виділена окремо саме тому, що це чиста безпека, а не UI.

## Як це працює

Користувач відкриває міні-апу з бота. Клієнт бере рядок `window.Telegram.WebApp.initData` (саме рядок, ніколи не `initDataUnsafe`) і надсилає його ОДИН раз на `POST /api/auth/telegram`. Сервер перевіряє підпис і повертає наш звичайний JWT — далі всі наявні ендпоінти працюють без жодних змін.

Перевірка робиться дослівно за схемою Telegram: розібрати рядок як query-string, вийняти `hash`, решту пар відсортувати за ключем і склеїти як `k=v` через перевід рядка; `secret_key = HMAC_SHA256(key="WebAppData", msg=<bot_token>)`; `computed = HMAC_SHA256(key=secret_key, msg=data_check_string)`; порівняти з `hash` У СТАЛИЙ ЧАС (у `webhooks.rs` така функція вже є — переїжджає, не пишеться заново). Далі: `auth_date` не старший за 300 секунд і не в майбутньому більш ніж на 60; поле `user` присутнє і має `id`.

Якщо `telegram_id` уже відомий — видаємо токен цьому користувачеві. Якщо ні — створюємо акаунт БЕЗ пошти: ім'я з Telegram, `email IS NULL`. Пошту попросимо пізніше, при першій купівлі, бо вона потрібна для листа з квитком — і це чесніший момент, ніж стіна на вході.

Бот-токен живе в конфізі за іменем ключа (`TELEGRAM_BOT_TOKEN`), значення видає власник поза кодом.

## Зміни в API

Новий модуль `api/src/telegram.rs`: `pub fn verify_init_data(init_data:&str, bot_token:&str, ttl_secs:i64) -> Result<TgUser, AppError>` (чиста функція, повністю юніт-тестована), `pub async fn auth_telegram(State, Json<{init_data:String}>) -> Result<Json<AuthResp>, AppError>`. Маршрут `POST /api/auth/telegram` (тільки POST, тільки тіло). `users.rs` отримує `find_by_telegram_id` і `create_telegram_user`. Конфіг: `TELEGRAM_BOT_TOKEN`, `TELEGRAM_INITDATA_TTL_SECS` (дефолт 300).

## Зміни в базі

Міграція 0045: `ALTER TABLE users ADD COLUMN telegram_id BIGINT, ADD COLUMN telegram_username TEXT`; `CREATE UNIQUE INDEX users_telegram_id_key ON users (telegram_id) WHERE telegram_id IS NOT NULL`; `ALTER TABLE users ALTER COLUMN email DROP NOT NULL`; `ADD CONSTRAINT users_identity_chk CHECK (email IS NOT NULL OR telegram_id IS NOT NULL OR google_sub IS NOT NULL)`. Обов'язковий супровід: перевірити КОЖНЕ місце, де `email` вважався NOT NULL — `auth::login`, `emailcode`, `passwords`, `tickets::gift_ticket`/`claim_ticket`, `gdpr::export_me`, `email` outbox, `admin::users` — і дописати гілку NULL. Це не косметика: саме тут ховається половина ризику міграції.

## Поверхні

**Веб.** Немає.

**Android.** Немає.

**Telegram Mini App.** Стартовий екран: поки триває обмін initData на токен — скелетон, не спінер на весь екран. Якщо міні-апу відкрито поза Telegram (немає `window.Telegram`) — зрозумілий екран «Відкрийте через нашого бота» з посиланням, а не мовчазна помилка.

## Критерії приймання (AC)

- Валідна initData з тестовим бот-токеном → 200 і робочий JWT, яким проходить `GET /api/me`.
- Та сама initData, надіслана вдруге через 10 хвилин → 401 (`auth_date` протух).
- Перше відкриття невідомим telegram_id створює рівно одного користувача з `email IS NULL` і `telegram_id` заповненим; друге відкриття тим самим id не створює другого.
- `verify_init_data` — чиста функція з юніт-тестами на 100% гілок (гейт покриття зелений).
- Користувач без email проходить `GET /api/me`, `GET /api/events`, `GET /api/me/tickets` без помилок серіалізації.
- `gdpr::export_me` і `admin::users` коректно віддають користувача з `email IS NULL` (регресія міграції).
- `POST /api/auth/telegram` без `TELEGRAM_BOT_TOKEN` у конфізі → 503 з ясним повідомленням у логах і БЕЗ спроби «пропустити перевірку».

## Що має бути відхилено (SEC)

- initData з підробленим `hash` → 401.
- initData, у якій змінено будь-яке поле (наприклад `user.id`) після обчислення hash → 401 — тест міняє по одному байту в кожному полі.
- initData, підписана ІНШИМ бот-токеном → 401.
- Відсутній `hash`, порожній рядок, `user` без `id`, дубльовані ключі в query-string, тіло понад 4 КБ → 401/400, жодного акаунта не створено.
- Порівняння hash не є звичайним `==` над рядками: control-presence тест — заміна на побайтове раннє повернення робить червоним іменований тест `hash_compare_is_constant_time_shape`.
- `auth_date` у майбутньому більш ніж на 60 с → 401 (інакше можна навічно «зарядити» токен наперед).
- `bot_token` ніколи не потрапляє у відповідь, у текст помилки й у лог (тест на відсутність підрядка в логах при кожній гілці помилки).
- initData, передана в query-параметрі GET → маршруту не існує (404/405), щоб підпис не осів у логах проксі й у Referer.
- Створений через initData акаунт має `role='USER'`, `is_organizer=false`, `blocked=false` — жодного успадкування прав.
- Перебір: `rate_events` бакет `tg_auth:<ip>` — 20/хв → 429; бакет `tg_auth:<telegram_id>` — 10/хв.
- `initDataUnsafe` не згадується в серверному коді взагалі (grep-гейт у CI).
