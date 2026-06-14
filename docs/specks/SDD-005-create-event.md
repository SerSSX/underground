# SDD-005 — Створення івенту (organizer)

**Backlog:** TASK-005  **Impacted UCs:** UC-006  **Impacted BR/WF:** BR-009, BR-011

## Scope / Non-goals
**Scope:** `POST /api/events` (authed → organizer_id = CurrentUser), `GET /api/me/events`; Android форма створення + «Мої події» + FAB.
**Non-goals:** edit/delete/stop-sales (TASK-005-B), постер-аплоад, продаж квитків.

## Design
**API** (`events.rs`):
- `CreateEventReq { title, description?, category, city?, venue_address?, starts_at?, ends_at?, price(Decimal), currency?, tickets_total(i32), cost_type? }`.
- `create_event(CurrentUser, State, Json)` → 201 `Event`; валідація → 422. Insert: `organizer_id`, `tickets_left = tickets_total`, `status='published'`, `currency` дефолт `UAH`, `cost_type` дефолт `paid`.
- `my_events(CurrentUser, State)` → `Vec<Event>` за `organizer_id`.
- Route: `/api/events` = `GET list_events` + `POST create_event`; `/api/me/events` = `GET my_events`.

**Android:**
- `data/EventApi`: `create(@Body CreateEventReq): EventDto`, `myEvents(): List<EventDto>`. `CreateEventReq` (snake_case через `@SerialName`, `price` як String, дата як ISO-рядок).
- `util/Categories` — пласка таксономія (~45) для пікера.
- `ui/create/CreateEventScreen` + `CreateEventViewModel` (форма, DatePicker+TimePicker, free-toggle, валідація кнопки).
- `ui/events/MyEventsScreen` + `MyEventsViewModel` (список своїх + «Створити подію»).
- `AppScaffold`: hoist `afishaVm`+`myEventsVm`; FAB «+»→`create`; таб «Квитки»→`MyEventsScreen`. Після створення — back + refresh обох.

## Acceptance Criteria
- **AC-1** `POST /api/events` без токена → 401; з токеном → 201, подія з `organizer_id = me`, `tickets_left = tickets_total`.
- **AC-2** Створена подія зʼявляється в `GET /api/events` і `GET /api/me/events`.
- **AC-3** Порожня назва/категорія, `tickets_total<0`, `price<0` → 422.
- **AC-4** Android: maker заповнює форму → «Опублікувати» → подія видима в Афіші й «Мої події».

## Security Acceptance Criteria
- **SEC-1** Створення лише автентифікованим (401 без токена); `organizer_id` береться з токена, не з тіла (анти-spoof).
- **SEC-2** Параметризовані запити; валідація вводу (422), без 500 на «брудний» ввід.
- **SEC-3** `GET /api/me/events` повертає лише події власника.

## Test Matrix
| AC | Unit | Integration | Manual |
|---|---|---|---|
| AC-1 | — | ✅ 401 / 201 + organizer | ✅ |
| AC-2 | — | ✅ зʼявляється у списках | ✅ |
| AC-3 | — | ✅ 422 | — |
| AC-4 | — | — | ✅ on device |
| SEC-1 | — | ✅ organizer з токена | — |
| SEC-3 | — | ✅ лише власник | — |
