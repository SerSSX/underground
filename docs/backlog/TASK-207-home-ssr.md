# TASK-207 — Home → SSR metadata + JSON-LD (INIT-SEO leaf-3)

**Status:** DONE (verified in-browser SSR)  **Parent:** TASK-199-F  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

## Raw Request
> Зняти `'use client'` з home (найлінкованіша сторінка) → SSR-metadata + структуровані дані для крадарів.
> Низько-ризиково: інтерактив лишається клієнтським островом.

## Refined Description
**Scope (presentation-only; без spec/behavior/API-змін):**
- `web/app/HomeClient.tsx` — поточна інтерактивна логіка home (`'use client'`, verbatim; секції/режими/save).
- `web/app/page.tsx` — тепер **server-компонент**: `export const metadata` (home title/description/canonical/OG) +
  server-fetch `/api/events` (через `serverApiBase`) → JSON-LD `ItemList` (top-30, crawlable) + `<HomeClient/>`.

**Non-goals:** SSR-рендер самого списку подій сервером (лишається клієнтським — Google рендерить JS); i18n-SSR; home-каруселі.

**Impacted:** без нових BR/UC/spec/API/міграцій.

## Verification (SSR curl + browser)
- Home HTTP 200; `<title>Underground — афіша, квитки, події</title>` (server metadata); `"@type":"ItemList"` JSON-LD у SSR-HTML.
- Інтерактив цілий: h1 + режим-chips + усі 4 секції (Для тебе/Незабаром/Цими вихідними/Популярне) + JSON-LD у DOM; 0 console-помилок.

## Note (dev-only)
Перемикання Next-сторінки client→server + додавання sibling-client-файлу лишає **stale Turbopack HMR-кеш** (фантомна
помилка «export metadata from 'use client'»). Код коректний; чиститься **рестартом dev-сервера** (перевірено: свіжий сервер — 0 помилок).
