# TASK-202 — SEO: JSON-LD Event + категорійні лендінги (INIT-SEO leaf-1)

**Status:** DONE (verified in-browser SSR)  **Parent:** TASK-199-F  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

> **Реалізовано + перевірено (SSR curl + browser):** `lib/seo.ts` (`eventJsonLd`/`itemListJsonLd`), `components/JsonLd.tsx`;
> `/events/[id]` рендерить JSON-LD Event (**verified:** LOUNA, offers 650 UAH InStock, location Place+address, startDate, image);
> новий `/c/[category]` SSR-лендінг (**verified:** /c/Party → 33 event-links + ItemList JSON-LD + метадані); `sitemap.ts` +
> категорійні URL (**verified:** /c/Concert,/c/Rock,…). **Bonus-fix:** `serverApiBase()` (`localhost`→`127.0.0.1`) — Node/undici
> резолвив `localhost`→IPv6 і **тихо ламав УСІ SSR-fetch** (event-OG-meta теж); тепер og:title подіє-специфічний. 0 console-помилок.

## Raw Request
> Органічний ріст: щоб події з'являлися у Google як rich results і мали лендінги під запити «rock події Київ».
> Наявне: подія SSR + OG/Twitter meta + sitemap усіх подій. Бракує: JSON-LD Event schema + категорійні лендінги.

## Refined Description
**Scope (additive presentation — без spec/behavior/API-змін):**
- **JSON-LD `Event`** на `web/app/events/[eventId]` (server-компонент): `<script type=application/ld+json>` зі
  schema.org/Event (name, startDate, location Place, image, offers price/currency/availability з `tickets_left`,
  organizer, url, eventStatus/attendanceMode). Робить подію придатною до Google rich event results.
- **Категорійні лендінги** `web/app/c/[category]` (SSR): `GET /api/events?category=X` → h1 + список посилань на події
  + `generateMetadata` (title/description/canonical) + JSON-LD `ItemList`. Драйвить органіку під категорійні запити.
- **Sitemap:** додати категорійні URL до наявного `sitemap.ts`.

**Non-goals:** зняття `'use client'` з home (окреме листя); гео-лендінги `/city/*`; hreflang/i18n-SEO; AMP; повний JSON-LD
BreadcrumbList/Organization-graph. Реюз наявних `/api/events?category` + sitemap.

**Impacted:** без нових BR/UC/міграцій/spec-змін (SEO-метадані — presentation). Верифікація в браузері: JSON-LD-скрипт у DOM,
category-лендінг рендерить події + метадані, 0 console-помилок.

## Estimation
**Level:** LOW — суто web-presentation поверх наявних SSR-fetch + `/api/events?category` + sitemap.

## Speck Reference
docs/specks/SDD-202-seo-jsonld-landing.md
