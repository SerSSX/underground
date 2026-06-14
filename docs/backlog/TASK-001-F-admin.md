# TASK-001-F — Web-адмінка (Next.js): вхід + налаштування платежів

**Status:** DONE (payments page; moderation — follow-up)
**Created:** 2026-06-12  **Updated:** 2026-06-12
**Parent:** TASK-001 / TASK-008  **Speck:** docs/specks/SDD-001-F-admin.md

## Verification (2026-06-12)
Next.js 15 (App Router, TS) у `admin/`: `/login` (admin→JWT у localStorage), `/payments` (GET/PUT payment-settings: mode, provider, комісія,
LS store/variant/api_key маскований). `npm run build` ✓ (6 сторінок, типи чисті). `npm run dev` на :3000, `/login` віддає UNDERGROUND/Увійти ✓.
**CORS** з origin :3000 → API :8080 дозволено (`allow-origin/methods/headers: *`). Admin-флоу (як у сторінок): login admin → GET (mode test, fee 1200) →
PUT (provider→lemonsqueezy, variant→var_777) ✓. Запуск: `scripts/admin-dev.sh`. Base URL — `NEXT_PUBLIC_API_BASE_URL`.
Відкрити: **http://localhost:3000** (admin@underground.dev / underground).
SEC: ключ лише `*_set` з бекенду, у формі password-поле, не шлеться без зміни; доступ лише admin-JWT (401/403).
Follow-up: модерація подій/користувачів/скарг + ескроу-дашборд; Playwright e2e; токен у httpOnly cookie (prod); контейнеризація в compose.

## Refined Description
**Scope (цей крок):** мінімальна Next.js (App Router, TS) адмінка в `admin/`: сторінка входу (admin email/пароль → app-JWT у localStorage),
сторінка **«Платежі»** поверх готового `GET/PUT /api/admin/payment-settings` (режим test/live, провайдер, **LS-ключі**/store/variant, комісія;
ключ маскується). Base URL API — через `NEXT_PUBLIC_API_BASE_URL` (не хардкод, §10). Запуск `npm run dev` на :3000 (CORS у API дозволено).
**Non-goals (далі):** модерація подій/користувачів/скарг; Playwright device-matrix; контейнеризація адмінки в compose; live-LS налаштування потоку.
**Dependencies:** TASK-008 backend (admin API).

## Estimation
**Level:** MEDIUM → цей крок: скелет + 2 сторінки (вхід, платежі).

## Speck Reference
docs/specks/SDD-001-F-admin.md
