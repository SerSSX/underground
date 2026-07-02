# SDD-202 — SEO: JSON-LD Event + категорійні лендінги

**Task:** TASK-202  **Status:** DONE (verified in-browser SSR)

## Acceptance Criteria
- **AC-1** `/events/[id]` віддає у SSR-HTML `<script type="application/ld+json">` зі schema.org/Event
  (name, url, startDate, location Place+address, offers price/priceCurrency/availability, organizer, image, description).
- **AC-2** `availability` = `InStock` коли `tickets_left>0`, інакше `SoldOut`.
- **AC-3** `/c/[category]` (SSR) рендерить published-події категорії (посилання на `/events/:id`) + `generateMetadata`
  (title/description/canonical/OG) + JSON-LD `ItemList`.
- **AC-4** `sitemap.xml` містить home + усі published-події + усі категорійні URL `/c/*`.
- **AC-5** Наявний плоский `/api/discover`, event-OG-meta та решта незмінні (additive).

## Security / Correctness ACs
- **SEC-1** Лендінги/JSON-LD показують лише публічні `published`-дані (той самий публічний `/api/events`); без auth/PII.
- **SEC-2** JSON-LD серіалізується через `JSON.stringify` (без інʼєкції сирого HTML у `dangerouslySetInnerHTML`).
- **FIX-1** SSR-fetch використовує `serverApiBase()` (`localhost`→`127.0.0.1`) — обхід Node/undici IPv6-first, що
  раніше тихо ламав усі SSR-fetch (event-OG-meta, sitemap). No-op на реальних prod-доменах.

## Test Matrix (verified via SSR curl)
| # | Вектор | Очікування | Результат |
|---|---|---|---|
| T1 | curl `/events/:id` | `"@type":"Event"` + повний блок | ✅ (LOUNA, offers 650 UAH InStock) |
| T2 | og:title | подіє-специфічний | ✅ «LOUNA … — Underground» |
| T3 | curl `/c/Party` | список подій + `ItemList` | ✅ 33 event-links + ItemList |
| T4 | curl `/sitemap.xml` | `/c/*` присутні | ✅ /c/Concert,/c/Rock,… |
| T5 | console | 0 помилок | ✅ |
