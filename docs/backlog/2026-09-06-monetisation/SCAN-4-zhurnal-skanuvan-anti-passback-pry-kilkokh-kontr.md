# SCAN-4 — Журнал сканувань, anti-passback при кількох контролерах, повноекранний вердикт

| Поле | Значення |
|---|---|
| **Фаза** | Фаза 3 — ролі й вхід |
| **Estimation** | M |
| **Status** | `INTAKE` |
| **Критичний шлях до першого платежу** | ні |
| **Depends on** | ROLE-1 |

## Навіщо

Наявний `staff::scan` уже серіалізує повторний скан через `FOR UPDATE OF t` — але ця властивість ніде не зафіксована тестом, а «вже використаний» без відповіді ХТО і КОЛИ провів безкорисний на дверях: контролер не може відрізнити передачу квитка через паркан від подвійного спрацювання власної камери. І немає журналу: після події неможливо сказати, скільки людей увійшло і хто їх пускав.

## Як це працює

Кожен скан пишеться в журнал — і успішний, і відхилений. Вердикт-екран стає повноекранним і читається за півсекунди в темряві: суцільний колір + велика іконка + один рядок тексту + ім'я власника квитка й тариф дрібнішим.

ЗЕЛЕНИЙ ✓ «Проходьте» — квиток дійсний, віброімпульс + короткий сигнал.
ЖОВТИЙ ⚠ «Уже входив» — з рядком «о 21:47, контролер Оля». Жовтий, а не червоний, свідомо: це не помилка системи, а ситуація, у якій рішення приймає людина.
ЧЕРВОНИЙ ✗ — «Невідомий QR», «Повернений», «Інша подія», «Подію скасовано», «Ще зарано» (двері відчиняються за N годин до початку, налаштовується організатором).

Угорі сканера постійний лічильник «увійшло 128 / 340» — він оновлюється після кожного скану і сам по собі є найкориснішою цифрою вечора.

Вікно ідемпотентності: повторний скан ТОГО САМОГО квитка ТИМ САМИМ контролером протягом 5 секунд повертає той самий зелений вердикт, а не жовтий. Камера часто спрацьовує двічі, і без цього кожен третій вхід виглядав би як спроба обману.

Власник і менеджер бачать живу стрічку входів (хто, коли, який контролер) — на великому екрані це фактично пульт вечора.

Розширює `staff.rs::scan` і `ScanTickets.kt`, не переписує їх.

## Зміни в API

`POST /api/events/:id/scan` — відповідь розширюється до `{valid, reason, event_title, ticket_id, holder_name, tier_name, first_scanned_at, first_scanned_by_name, checked_in, tickets_sold}`; тіло приймає необов'язкові `device_id` і `client_scanned_at`. Нові коди `reason`: `event_cancelled`, `too_early`. Нове: `GET /api/events/:id/checkins?since=&limit=` (Cap::ViewGuests) → стрічка входів; `GET /api/events/:id/checkin-stats` → `{sold, checked_in, rejected, by_staff:[{name,count}]}`. `ALTER` поведінки: `events` отримує `doors_open_hours_before INT` (дефолт 6) — редагується в формі події.

## Зміни в базі

Міграція 0043: `CREATE TABLE checkins (id UUID PK, event_id UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE, ticket_id UUID REFERENCES tickets(id) ON DELETE SET NULL, order_id UUID REFERENCES orders(id) ON DELETE SET NULL, scanned_by UUID REFERENCES users(id) ON DELETE SET NULL, device_id TEXT, verdict TEXT NOT NULL, source TEXT NOT NULL DEFAULT 'online' CHECK (source IN ('online','offline')), client_scanned_at TIMESTAMPTZ, scanned_at TIMESTAMPTZ NOT NULL DEFAULT now())`; `CREATE UNIQUE INDEX checkins_one_valid ON checkins (ticket_id) WHERE verdict='valid'` — саме цей індекс, а не код, робить anti-passback властивістю бази; `CREATE INDEX checkins_event_time_idx ON checkins (event_id, scanned_at DESC)`. `ALTER TABLE events ADD COLUMN doors_open_hours_before INT NOT NULL DEFAULT 6`.

## Поверхні

**Веб.** `/organizer/[eventId]/live` — жива панель: великий лічильник «увійшло / продано», темп входів по хвилинах, стрічка останніх 50 сканів з ім'ям контролера, фільтр «лише відхилені». Автооновлення поллінгом раз на 5 с (SSE/WS не вводимо заради однієї сторінки).

**Android.** `ScanTickets.kt` розширюється: діалог замінюється на повноекранний `ScanVerdictScreen` (колір фону з `Color.kt`, іконка, текст, ім'я, тариф, рядок «уже входив о …»), верхній лічильник, віброімпульс через `HapticFeedback` (різний для зеленого й червоного), автозакриття зеленого через 1.2 с і ручне закриття жовтого/червоного — щоб контролер мусив свідомо відпустити спірний випадок.

**Telegram Mini App.** Вердикт той самий (спільний компонент з веб-частиною), сканування — через `Telegram.WebApp.showScanQrPopup` (див. TMA-14). Жива панель доступна і в TMA.

## Критерії приймання (AC)

- Два одночасні `POST /api/events/:id/scan` одного квитка з двох різних сесій дають рівно один `valid`; другий — `already_used` з непорожніми `first_scanned_at` і `first_scanned_by_name`.
- Control-presence: видалення `FOR UPDATE OF t` з `staff::scan` робить червоним іменований тест `concurrent_scan_admits_once` (тест ганяє N паралельних сканів у транзакціях).
- Другий скан того самого квитка тим самим користувачем через 2 с → `valid` (вікно ідемпотентності); через 10 с → `already_used`.
- Скан за 12 годин до `starts_at` при `doors_open_hours_before=6` → `too_early`, квиток лишається `active`.
- Скан квитка події зі `status='cancelled'` → `event_cancelled`, квиток лишається `active`.
- Кожен скан (включно з `not_found`) створює рядок у `checkins`; `checkins_one_valid` не дає з'явитися другому `valid` на той самий квиток навіть при прямому INSERT.
- `GET /api/events/:id/checkin-stats` повертає `checked_in`, що дорівнює `COUNT(*) FROM checkins WHERE verdict='valid'`.

## Що має бути відхилено (SEC)

- Відповідь `scan` не містить email, телефону, `order_id` покупця чи суми — лише ім'я, тариф і час. Контролер бачить рівно те, що потрібно на дверях.
- `GET /api/events/:id/checkins` і `/checkin-stats` для ролі Scanner → 403.
- Скан квитка ІНШОЇ події → `wrong_event`, і квиток НЕ позначається використаним у жодній події.
- Підроблений `device_id` у тілі запиту не впливає на вердикт — anti-passback тримається на `ticket_id`, а `device_id` лише пишеться в журнал.
- `client_scanned_at` у майбутньому або старіший за 24 год ігнорується (у журнал іде серверний час), щоб журнал не можна було переписати.
- Прострочений (TTL 120 с) або підписаний іншим секретом QR → `not_found`, без розкриття, чи такий квиток існує.
- Сирий `qr_token` зі сканера не потрапляє в логи й у `audit` (логується лише `ticket_id` після успішної верифікації).
- `GET /api/events/:id/checkins` іншої події → 403/404 (перевірка через `require_cap`, не через довіру до `event_id` у запиті).
