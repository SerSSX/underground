# TASK-208 — Web notifications inbox (замикає retention-петлю на web)

**Status:** DONE (verified in-browser)  **Parent:** TASK-199-D  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

## Raw Request
> Функціональний розрив: web НЕ мав inbox-у сповіщень (Android має, tappable). Тому `event_reminder`/`review_prompt`
> (TASK-204/205) на web не було де побачити й натиснути → retention-петля «залиш відгук» неповна. Закрити паритетом.

## Refined Description
**Scope (FE-only; API-ендпоінти вже є: `GET /api/me/notifications`, `POST /api/me/notifications/read`):**
- `web/lib/api.ts` — `NotificationDto` + `myNotifications()` + `markNotificationsRead()`.
- `web/app/inbox/page.tsx` (client) — список сповіщень (title/body/дата, unread-акцент), **tap → `/events/:event_id`**
  коли є подія; mark-read при завантаженні; auth-guard (redirect на login).
- `web/app/components/TopNav.tsx` — лінк «Вхідні» для залогінених (між «Мої квитки» і «Кабінет»).

**Non-goals:** unread-badge у навбарі; realtime; повідомлення-чат (окремий Android-таб); фільтри за kind.

**Impacted:** без нових BR/UC/spec/API/міграцій. Presentation-only, паритет з Android `AlertsScreen`.

## Verification (browser)
- `/inbox` як `ctrl` → навбар має «Вхідні» ✓; heading «Сповіщення» ✓; seeded `review_prompt`(LOUNA) рендериться як
  clickable button ✓; **клік → `/events/11111111-…` (h1 «LOUNA — 10 років гурту»)** ✓ — retention-петля замкнена.
- 0 console-помилок. Паритет із Android `AlertsScreen.onOpenEvent`.
