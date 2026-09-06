# ROLE-2 — Запрошення, відкликання і перелік персоналу; org-рівнева команда; багаторазовий інвайт для зміни на дверях

| Поле | Значення |
|---|---|
| **Фаза** | Фаза 3 — ролі й вхід |
| **Estimation** | M |
| **Status** | `INTAKE` |
| **Критичний шлях до першого платежу** | ні |
| **Depends on** | ROLE-1 |

## Навіщо

`staff.rs` уже вміє один сценарій: одноразовий токен на одну подію з роллю 'scan'. Немає ні списку персоналу, ні відкликання, ні org-рівня, ні вибору ролі. Організатор із трьома контролерами на дверях сьогодні мусить згенерувати три посилання і не має де подивитись, кому вони дісталися. Плюс наявний `ON CONFLICT (event_id,user_id) DO NOTHING` в `accept_invite` — це справжня діра: після відкликання людину неможливо додати назад, а підвищення ролі мовчки не спрацьовує.

## Як це працює

Організатор відкриває «Персонал» події і бачить таблицю: ім'я, роль, хто запросив, коли додано, скільки сканів зробив, кнопка «Відкликати». Кнопка «Запросити» дає вибір ролі (Контролер / Менеджер — менеджеру доступний лише Контролер) і термін дії посилання. Посилання за замовчуванням ОДНОРАЗОВЕ. Для дверей є другий варіант — «посилання на зміну»: до 20 використань і TTL до 24 годин, доступний лише власнику. Це свідомий компроміс: черга на вході не чекатиме, поки організатор нагенерує п'ять окремих лінків, а вузьке вікно й ліміт роблять витік посилання малоцінним.

Окремий екран «Моя команда» на рівні організатора: ті самі дії, але роль діє на всі його події. Типовий випадок — постійний партнер, який завжди на дверях.

Відкликання миттєве: людина, яка тримає відкритий сканер, на наступному скані отримує «Доступ відкликано» і екран закривається.

Розширює `staff.rs` і таблиці з ROLE-1; нових концепцій не вводить.

## Зміни в API

`POST /api/events/:id/staff/invite` — тіло розширюється до `{permission: "scan"|"manager", max_uses?: 1..20, ttl_hours?: 1..168}` (дефолти 'scan', 1, 72); відповідь додає `permission`, `max_uses`, `deep_link`. Нове: `POST /api/organizer/staff/invite {permission, max_uses?, ttl_hours?}`; `GET /api/events/:id/staff` → `[{user_id,name,avatar_url,permission,scope:"event"|"organizer",invited_by_name,created_at,scans_count}]`; `DELETE /api/events/:id/staff/:user_id`; `GET /api/organizer/staff`; `DELETE /api/organizer/staff/:user_id`; `GET /api/events/:id/staff/invites` (активні непогашені посилання) + `DELETE /api/staff/invites/:id` (анулювати). `POST /api/staff/accept` розширюється: приймає і event-, і org-інвайт, у відповіді повертає `{scope, permission, event_id?, organizer_name}` — щоб клієнт сказав людині, що саме вона отримала. `GET /api/me/staff/events` додає в кожен рядок `role`.

## Зміни в базі

Міграція 0041: `ALTER TABLE staff_invites ADD COLUMN organizer_id UUID REFERENCES users(id) ON DELETE CASCADE`, `ALTER TABLE staff_invites ALTER COLUMN event_id DROP NOT NULL`, `ADD CONSTRAINT staff_invites_scope_chk CHECK ((event_id IS NOT NULL) <> (organizer_id IS NOT NULL))`, `ADD COLUMN max_uses INT NOT NULL DEFAULT 1 CHECK (max_uses BETWEEN 1 AND 20)`, `ADD COLUMN used_count INT NOT NULL DEFAULT 0`, `ADD COLUMN revoked_at TIMESTAMPTZ`. Виправлення діри в `accept_invite`: `ON CONFLICT (event_id,user_id) DO UPDATE SET permission = EXCLUDED.permission, revoked_at = NULL, invited_by = EXCLUDED.invited_by` — з умовою, що пониження ролі через ON CONFLICT дозволене лише коли запрошувач = власник.

## Поверхні

**Веб.** `/organizer/[eventId]/staff` — таблиця персоналу, діалог запрошення з вибором ролі й типу посилання, кнопка «Копіювати посилання» + QR цього посилання на екран (контролер сканує з телефона — швидше, ніж пересилати в месенджері). `/organizer/team` — org-рівнева команда. Список активних посилань з кнопкою «Анулювати».

**Android.** Розділ «Персонал» у `EventAdminScreen`: список з свайпом-відкликанням, FAB «Запросити» → системний share-лист з `underground://staff/<token>?e=<id>&t=<title>` (deep link уже обробляється в `MainActivity.extractStaffInvite`). Екран «Моя команда» в кабінеті. `AcceptStaffScreen` розширюється: показує, яку саме роль і на що людина отримує, до підтвердження.

**Telegram Mini App.** Ті самі два екрани; замість share-листа — `Telegram.WebApp.switchInlineQuery` або кнопка «Надіслати в чат», бо переслати посилання в Telegram — рідний жест для цієї поверхні.

## Критерії приймання (AC)

- Інвайт з `max_uses=3` приймається трьома різними користувачами; четвертий отримує 409 і `used_count` лишається 3.
- Інвайт з `max_uses=3`, прийнятий двічі ОДНИМ користувачем, витрачає рівно одне використання (другий раз — ідемпотентно, `used_count` не росте).
- `DELETE /api/events/:id/staff/:user_id` → `revoked_at` проставлено; наступний `POST /api/events/:id/scan` тим самим токеном → 403.
- Відкликана людина, яка знову приймає новий інвайт, отримує доступ (перевіряє виправлення `ON CONFLICT DO NOTHING`).
- `GET /api/events/:id/staff` для менеджера повертає список без `email` і без `invited_by` тих, кого запросив власник поза його видимістю — лише ім'я й роль.
- `GET /api/me/staff/events` контролера з org-роллю повертає всі опубліковані події цього організатора, включно зі створеною після видачі ролі.
- Анулювання посилання (`DELETE /api/staff/invites/:id`) робить його непридатним негайно: наступний `POST /api/staff/accept` з ним → 404.

## Що має бути відхилено (SEC)

- Менеджер створює інвайт з `permission='manager'` → 403.
- Менеджер створює інвайт з `max_uses>1` → 403 (багаторазові посилання — лише власник).
- Прострочений інвайт → 409, статус переводиться в 'expired', доступ НЕ видається.
- Анульований інвайт (`revoked_at`) → 404, навіть якщо `status='pending'` і TTL живий.
- Сирий `invite_token` не з'являється в логах, у відповіді `GET /api/events/:id/staff/invites` (там лише id, роль, лічильники) і в тексті помилок.
- Перебір токенів: `POST /api/staff/accept` обмежений через наявну `rate_events` (бакет `staff_accept:<ip>` і `staff_accept:<user_id>`), 10 спроб/хв → 429.
- Інвайт на скасовану або видалену подію → 404, рядок `event_staff` не створюється.
- Гонка: два одночасні `accept` останнього використання посилання `max_uses=1` дають рівно один доступ (`FOR UPDATE` на рядку інвайта, `used_count` атомарний).
- `DELETE /api/organizer/staff/:user_id`, викликаний менеджером → 403 (org-команда — власницька).
- Відкликання персоналу пишеться в `audit` (є `audit.rs`): хто, кого, коли, з якої ролі.
