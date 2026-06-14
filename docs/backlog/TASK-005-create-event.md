# TASK-005 — Створення івенту (organizer)

**Status:** DONE
**Created:** 2026-06-12  **Updated:** 2026-06-12
**Parent:** TASK-001 (UC-006)  **Speck:** docs/specks/SDD-005-create-event.md

## Verification (2026-06-12)
Backend: `POST /api/events` (organizer = CurrentUser), `GET /api/me/events`; 27 тестів зелені (events_create: 401 без токена,
201 + organizer + tickets_left=total, зʼявляється у public+me списках, 422 на blank/negative). clippy/fmt чисто. Curl-смоук як maker → 201.
Android: екран «Нова подія» (назва/опис/категорія-дропдаун з таксономії ~48/місто/адреса/DatePicker+TimePicker/free-тогл/ціна/квитки),
таб «Мої» (список своїх + «Створити»), FAB «+». **Перевірено на moto g05:** maker увійшов → FAB → форма → Дата 2026-06-20 + Free → Опублікувати →
повернення на «Мої події» (оновлено) → подія «Live» **зʼявилась у публічній Афіші** (Київ · 0 UAH перед 06-21). API: maker-подій 21→22.
SEC: organizer лише з токена (не з тіла); 401 без авторизації; параметризовані запити; валідація 422.
Follow-up: безкоштовні події показувати «Безкоштовно» замість «0 UAH»; edit/delete/stop-sales (TASK-005-B); постер-аплоад.

## Nav fix (2026-06-12, на запит «без тупіків»)
Екран створення отримав `TopAppBar` зі стрілкою «назад» (`onBack`→`popBackStack`) — раніше був лише системний жест. Перевірено на moto g05: відкрив форму → стрілка → повернення на «Мої події». Аудит навігації: Деталі (стрілка) ✓, Створення (топ-бар) ✓, таби (нижня нав) ✓ — тупіків немає.

## Raw Request
> «Дороблюй додаток фіча за фічею» → можливість як івент-мейкер створити подію (UC-006), щоб вона зʼявилась в Афіші.

## Refined Description
**Scope:** автентифікований `POST /api/events` (owner = CurrentUser, status `published`, `tickets_left = tickets_total`);
`GET /api/me/events` (свої події). Android: екран створення (назва, опис, категорія з таксономії, місто, адреса, дата/час,
платно/безкоштовно + ціна, к-сть квитків) → публікація → подія в Афіші; таб «Мої події» (список своїх) + FAB «+».
**Non-goals:** редагування/видалення/stop-sales (TASK-005-B); завантаження постера (поки градієнт-fallback); продаж квитків (TASK-006).
**Impacted UCs:** UC-006. **Impacted BR/WF:** BR-009, BR-011. **Dependencies:** TASK-002 (auth).

## Estimation
**Level:** MEDIUM — як одне ціле (форма + 2 ендпоінти).

## Speck Reference
docs/specks/SDD-005-create-event.md
