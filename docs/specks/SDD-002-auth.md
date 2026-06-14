# SDD-002 — Auth (email/пароль) + сесія

**Backlog:** TASK-002  **Impacted UCs:** UC-001  **Impacted BR/WF:** BR-007, BR-016, BR-017, WF-Auth

## Scope / Non-goals
**Scope:** email+пароль register/login/me, argon2id, HS256 JWT, `CurrentUser` extractor; Android auth-екран + сесія + gate + профіль + вихід; сід user+maker.
**Non-goals:** Google OAuth (під-таск); онбординг prefs (TASK-002-B); refresh-токени.

## Design
**API** (`api/src/auth.rs`, `users.rs`):
- `POST /api/auth/register {email,password,name}` → 201 `{token,user}`; email зайнятий → 409; невалідне → 422.
- `POST /api/auth/login {email,password}` → 200 `{token,user}`; невірно → 401 (без розкриття, що саме).
- `GET /api/me` (Bearer) → 200 `user`; без/невалідний токен → 401.
- argon2id `hash_password`/`verify_password` (constant-time); пароль ніколи не в логах/відповіді (BR-016).
- JWT HS256, `JWT_SECRET`, claim `sub`=user_id, TTL 7д; `CurrentUser` extractor декодує Bearer.
- Міграція `0003`: `users.password_hash TEXT`. `User` DTO без `password_hash`.

**Android** (`auth/`):
- `AuthScreen` (перемикач Вхід/Реєстрація, email+пароль[+ім'я], помилки); кнопка Google — заглушка «скоро».
- `TokenStore` (EncryptedSharedPreferences); `AuthInterceptor` додає `Authorization: Bearer`.
- `AuthViewModel` + gate в `MainActivity`: токен є → app, нема → AuthScreen; вихід чистить токен.
- Профіль показує реального юзера (`GET /api/me`): ім'я, email, роль; кнопка «Вийти».

**Seed:** `scripts/seed-users.sh` — реєструє `user@underground.dev` і `maker@underground.dev` (пароль `underground`) через API (ідемпотентно: 409 = вже є), потім `UPDATE events SET organizer_id = maker`.

## Acceptance Criteria
- **AC-1** register створює юзера (argon2-хеш у БД, не плейн), повертає валідний JWT.
- **AC-2** login вірними кредами → токен; невірними → 401.
- **AC-3** `GET /api/me` з токеном → юзер; без — 401.
- **AC-4** Android: без токена показує AuthScreen; після входу — застосунок + профіль реального юзера; вихід повертає на AuthScreen.
- **AC-5** Тест-акаунти user+maker існують; maker — організатор сід-подій.

## Security Acceptance Criteria
- **SEC-1** Пароль зберігається лише як argon2id-хеш; не в логах/відповідях (BR-016).
- **SEC-2** Дублікат email → 409 (без витоку існування через тайминг — однакова обробка).
- **SEC-3** Підроблений/прострочений/відсутній JWT → 401 (не 500); `sub` не-UUID → 401.
- **SEC-4** Валідація вводу: email-формат, пароль ≥8 символів, ім'я непорожнє → 422.

## Test Matrix
| AC | Unit | Integration | Curl/Manual |
|---|---|---|---|
| AC-1 | ✅ hash/verify, jwt encode/decode | ✅ register | ✅ |
| AC-2 | — | ✅ login ok/bad | ✅ |
| AC-3 | — | ✅ me with/without token | ✅ |
| AC-4 | — | — | ✅ on device |
| AC-5 | — | ✅ seed users | ✅ |
| SEC-1 | ✅ (hash != plain) | ✅ (no hash in resp) | — |
| SEC-2 | — | ✅ duplicate→409 | — |
| SEC-3 | ✅ decode invalid | ✅ me forged/expired→401 | ✅ |
| SEC-4 | ✅ validation | ✅ 422 | — |
