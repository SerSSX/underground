# SDD-001-F — Web-адмінка (вхід + налаштування платежів)

**Backlog:** TASK-001-F  **Parent:** TASK-001 / TASK-008

## Scope / Non-goals
**Scope:** Next.js (App Router, TS) у `admin/`: `/login` (admin → JWT у localStorage), `/payments` (GET/PUT admin payment-settings).
Base URL — `NEXT_PUBLIC_API_BASE_URL`. Темна тема.
**Non-goals:** модерація подій/користувачів/скарг/ескроу; Playwright matrix; контейнеризація; live-LS потік.

## Design
- `lib/api.ts` — `login/getSettings/updateSettings`, токен у localStorage, `authHeaders`.
- `/login` — форма (дефолт `admin@underground.dev`), помилка 401 «невірний пароль».
- `/payments` — gate (нема токена→/login); форма: mode (test/live), provider, комісія bps, LS store/variant/api_key (маскований; порожнє поле = не змінювати); 403 «потрібні права адміністратора»; «Вийти».
- API CORS дозволено (CorsLayer::permissive).

## Acceptance Criteria
- **AC-1** `/login` логінить admin → редірект на `/payments`; невірний пароль → помилка.
- **AC-2** `/payments` показує поточні налаштування (mode/provider/fee/store/variant; ключ як «встановлено/ні»).
- **AC-3** Збереження PUT оновлює; нове значення ключа надсилається лише якщо поле непорожнє.
- **AC-4** Без токена `/payments` → редірект `/login`; не-admin токен → повідомлення про права.

## Security Acceptance Criteria
- **SEC-1** API-ключ ніколи не приходить з бекенду (лише `*_set`); у формі — password-поле, не передається назад без зміни.
- **SEC-2** Доступ до даних — лише з валідним admin-JWT (бекенд 401/403); токен у localStorage (dev-рівень; для prod — httpOnly cookie, follow-up).
- **SEC-3** Base URL з env (не хардкод).

## Test Matrix
| AC | Перевірка |
|---|---|
| AC-1..4 | ✅ ручна (браузер): вхід admin → /payments → зміна mode/fee/ключ → Зберегти ✓; logout; не-admin → 403 повідомлення |
| build | ✅ `npm run build` без помилок типів |
> Playwright e2e — follow-up (device-matrix §7.3).
