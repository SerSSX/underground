# ROLE-1 — Модель ролей поверх event_staff: owner / manager / scan, event-scoped + organizer-scoped

| Поле | Значення |
|---|---|
| **Фаза** | Фаза 3 — ролі й вхід |
| **Estimation** | L |
| **Status** | `INTAKE` |
| **Критичний шлях до першого платежу** | ні |
| **Depends on** | — |

## Навіщо

Сьогодні `event_staff.permission` має рівно одне значення 'scan', а `staff.rs::can_scan` — єдина перевірка доступу в усьому персональному контурі. Власник вимагає помічників, які «допомагають керувати», а не лише пускають людей. Без явної матриці дозволів кожен наступний ендпоінт буде вирішувати сам, і рано чи пізно менеджер отримає доступ до грошей. Це фундамент, від якого залежать усі інші задачі фази 3-5.

## Як це працює

Ролей рівно три, більше не треба. ВЛАСНИК (events.organizer_id, рядка в таблиці не має) — може все. МЕНЕДЖЕР — редагує опис, афішу, час і місце події, керує промокодами, бачить список гостей і агреговану виручку, зупиняє продаж, запрошує контролерів, сканує сам. НЕ може: міняти ціну, валюту й політику повернень, скасовувати подію, повертати гроші, бачити банківські реквізити й виплати, запрошувати іншого менеджера. КОНТРОЛЕР ('scan', наявне значення) — лише сканує і бачить вердикт; списку гостей не бачить взагалі.

Роль видається у двох масштабах. НА ПОДІЮ — як зараз (`event_staff`). НА ОРГАНІЗАТОРА — нова таблиця `organizer_staff`: людина стає менеджером чи контролером усіх подій цього організатора, теперішніх і майбутніх, і її не треба перезапрошувати на кожну вечірку. Ефективна роль = найвища з двох (org-рівень і event-рівень).

Правило ескалації одне й просте: запрошувач не може видати роль, вищу за власну. Менеджер запрошує лише контролерів. Відкликаний персонал (`revoked_at IS NOT NULL`) втрачає доступ у ту саму секунду, а не після протухання його JWT — перевірка робиться на кожному запиті.

Розширює `staff.rs`, не створює заново: `can_scan` замінюється на `effective_role(db, event_id, user_id) -> Role` і `require_cap(db, event_id, user, Cap)`, а всі наявні виклики переводяться на них.

## Зміни в API

У `api/src/staff.rs`: `pub enum Role { Owner, Manager, Scanner, None }`, `pub enum Cap { Scan, ViewGuests, EditEvent, EditMoney, ManagePromos, CancelEvent, Refund, RequestRefund, ViewPayouts, InviteScanner, InviteManager, ExportPii }`, `pub async fn effective_role(db,&event_id,&user_id) -> Result<Role, AppError>`, `pub async fn require_cap(db, event_id, user_id, cap) -> Result<Role, AppError>` (403 при відмові). `can_scan` видаляється, `staff::scan` і `staff::my_staff_events` переводяться на `effective_role`. Наявні `events::update_event`, `events::delete_event`, `promos::*`, `organizer::event_tickets` замінюють свою власну перевірку `organizer_id = caller` на `require_cap` (додає доступ менеджеру там, де матриця дозволяє). Новий `GET /api/events/:id/my-role` → `{role, caps:[..]}` — щоб клієнти малювали лише те, що дозволено, і не вгадували.

## Зміни в базі

Міграція 0040: `ALTER TABLE event_staff ADD CONSTRAINT event_staff_permission_chk CHECK (permission IN ('scan','manager'))`; `ALTER TABLE event_staff ADD COLUMN revoked_at TIMESTAMPTZ`; `CREATE INDEX event_staff_user_idx ON event_staff (user_id) WHERE revoked_at IS NULL`. Нова таблиця `organizer_staff (id UUID PK, organizer_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE, user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE, permission TEXT NOT NULL CHECK (permission IN ('scan','manager')), invited_by UUID REFERENCES users(id) ON DELETE SET NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), revoked_at TIMESTAMPTZ, UNIQUE (organizer_id, user_id))` + індекс `(user_id) WHERE revoked_at IS NULL`. `ALTER TABLE staff_invites ADD COLUMN permission TEXT NOT NULL DEFAULT 'scan' CHECK (permission IN ('scan','manager'))`.

## Поверхні

**Веб.** Немає власного екрана — лише наслідок: кабінет організатора малює пункти меню за `caps` з `GET /api/events/:id/my-role`. Пункти, недоступні ролі, НЕ показуються сірими, а зникають — сірі пункти породжують звернення в підтримку.

**Android.** Те саме: `EventAdminScreen` бере `caps` один раз і ховає розділи «Гроші», «Персонал», «Скасувати подію» для контролера й менеджера відповідно.

**Telegram Mini App.** Те саме, з тим самим `caps`-джерелом. Окремої логіки прав у TMA немає за рішенням ROLE-1 — інакше з'явиться другий, слабший контур авторизації.

## Критерії приймання (AC)

- `effective_role` повертає Owner для `events.organizer_id`, Manager для рядка `event_staff.permission='manager'` без `revoked_at`, Scanner для 'scan', None для стороннього користувача.
- Людина з `organizer_staff.permission='manager'` отримує Manager на ПОДІЮ, створену цим організатором ПІСЛЯ видачі org-ролі, без жодної додаткової дії.
- Ефективна роль при org='scan' і event='manager' дорівнює Manager; при org='manager' і event='scan' — теж Manager.
- `UPDATE event_staff SET revoked_at = now()` → наступний `POST /api/events/:id/scan` тим самим (непротухлим) JWT повертає 403.
- `GET /api/events/:id/my-role` для контролера повертає `caps` рівно `["scan"]`.
- Тест control-presence: видалення виклику `require_cap` з `events::update_event` робить червоним іменований тест `scanner_cannot_edit_event`.
- Гейт покриття 100% line лишається зеленим (`scripts/ci.sh`).

## Що має бути відхилено (SEC)

- Контролер робить `PUT /api/events/:id` → 403, подія не змінена.
- Менеджер робить `PUT /api/events/:id` з полем `price` (або `currency`, або будь-яким полем політики повернень) → 403 і ЖОДНЕ поле запиту не застосовано, навіть дозволені. Перевірка полева, не тільки ендпоінтна.
- Менеджер робить `DELETE /api/events/:id` (скасування) → 403.
- Менеджер робить інвайт з `permission='manager'` → 403 (ескалація привілею).
- Контролер робить `GET /api/events/:id/tickets` → 403 (список гостей — це PII).
- Org-рівневий менеджер організатора A звертається до події організатора B → 403.
- Рядок з `revoked_at` не враховується в `effective_role` у ЖОДНОМУ з трьох масштабів (event, organizer, invite).
- Контролер робить `GET /api/organizer/payouts` → бачить ЛИШЕ свої власні виплати як організатора, а не виплати того, у кого він контролер.
- `GET /api/events/:id/my-role` для стороннього користувача повертає `role: "none"` і порожні `caps`, а не 404 з витоком факту існування події (подія й так публічна — але відповідь не має містити нічого приватного).
