# TASK-002 — Auth (email/пароль) + сесія

**Status:** DONE
**Created:** 2026-06-12  **Updated:** 2026-06-12
**Parent:** TASK-001 (UC-001)  **Speck:** docs/specks/SDD-002-auth.md

## Verification (2026-06-12)
Backend: 24 тести зелені (auth unit: hash/verify, jwt, email-validation; integration: register→me→login, duplicate→409,
wrong-pass→401, me без/невалідний токен→401, validation→422). clippy/fmt чисто. Curl-смоук: login→token, `/api/me`→юзер, no-token→401.
Тест-акаунти створено: **user@underground.dev** / **maker@underground.dev** (пароль `underground`); maker — організатор сід-подій.
Android: екран входу/реєстрації, EncryptedSharedPreferences токен, AuthInterceptor, gate. **Перевірено на moto g05:**
чистий екран входу → ввід `user@underground.dev` → вхід → застосунок → таб Профіль показує реального юзера (ім'я/email/роль) + «Вийти».
SEC: пароль лише argon2-хеш (не в JSON/логах), 401 на підробку токена, 422 на невалідний ввід, cleartext лише loopback.
Known-issue (cosmetic): на AuthScreen рендериться лише підзаголовок BrandHeader, слово «UNDERGROUND» не видно (рендериться ок на Афіші) —
схоже на взаємодію `headlineMedium`+`letterSpacing` всередині `verticalScroll`; функціонал не зачіпає. → follow-up.
Follow-up: виправити wordmark на AuthScreen; Google OAuth (потребує GOOGLE_CLIENT_ID); онбординг інтересів/міста (TASK-002-B).

## Raw Request
> «Дороблюй додаток і тестуй фічу за фічею. Нароби тестових акаунтів юзера і івент-мейкера. Щоб був нормальний додаток.»

## Refined Description
**Scope:** email+пароль автентифікація (UC-001, BR-016/017): `register`, `login`, `me`; argon2id-хеш; app-JWT (HS256);
`CurrentUser`-extractor; Android — екран входу/реєстрації, безпечне збереження токена, gate (auth→app), профіль реального юзера, вихід.
Сід тестових акаунтів **user** + **event-maker** (maker володіє сід-подіями).
**Non-goals:** Google OAuth (потребує `GOOGLE_CLIENT_ID` — окремий під-таск; лишаємо кнопку-заглушку); онбординг інтересів/міста
(TASK-002-B); refresh-токени (короткий TTL поки що).
**Impacted UCs:** UC-001. **Impacted BR/WF:** BR-007, BR-016, BR-017, WF-Auth. **Dependencies:** TASK-001-D.

## Estimation
**Level:** MEDIUM → реалізуємо як одне ціле (auth-ядро); підтверджено користувачем («фіча за фічею»).

## Speck Reference
docs/specks/SDD-002-auth.md
