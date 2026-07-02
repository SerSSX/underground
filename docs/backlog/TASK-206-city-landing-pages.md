# TASK-206 — Гео-лендінги `/city/[city]` (INIT-SEO leaf-2)

**Status:** DONE (verified in-browser SSR)  **Parent:** TASK-199-F  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

## Raw Request
> Гео-SEO: лендінги під запити «події в Києві». Дзеркало вже-збудованих категорійних `/c/[category]` (TASK-202),
> фільтр за містом. Реюз `/api/events?city=` + `itemListJsonLd` + sitemap.

## Refined Description
**Scope (additive presentation — без spec/behavior/API-змін):**
- `web/app/city/[city]` (SSR): `GET /api/events?city=X` (через `serverApiBase`) → h1 + список посилань на події +
  `generateMetadata` (title «Події в місті X — Underground» / description / canonical / OG) + JSON-LD `ItemList`.
  Порожній стан із лінком на афішу.
- **Sitemap:** додати `/city/*` URL, похідні від distinct-міст подій (`new Set(events.map(e=>e.city))`), поряд із `/c/*`.

**Non-goals:** канонізація варіантів міста (Kyiv/Київ — окремо); hreflang; district-лендінги; home→SSR (окреме листя).

**Impacted:** без нових BR/UC/spec/API/міграцій. Presentation-only.

## Verification (SSR curl + browser)
- `/city/Kyiv` → `<title>Події в місті Kyiv — Underground</title>`, 117 event-links, `ItemList` JSON-LD ✓; 0 console-помилок.
- `/city/Narnia` → порожній стан «Поки немає подій у цьому місті» ✓.
- `sitemap.xml` → `/city/Kyiv`, `/city/Berlin`, `/city/Tokyo`, `/city/%D0%9A…` (Київ, URL-encoded) ✓.
