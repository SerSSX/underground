# Underground v2 — User Flows (посторінково: екрани, поля, функції)

> **Версія:** 2.0 · **Дата:** 2026-06-29 · **Аудиторія:** dev + design
> **Принцип №1 (Product-first):** продукт працює ДО реального платіжного провайдера — через `NoopProvider` (free/RSVP), `TestProvider` (admin mark-as-paid), і чистий seam `PaymentProvider`.
> **Принцип №2 (Provider-agnostic):** домен квитків + ескроу — наш і провайдер-нейтральний. Провайдер виконує лише `charge / hold / transfer(payout) / refund / verify_webhook`. Жоден екран продукту не знає про конкретного провайдера; checkout повертає `provider_action {kind, url?}`.
> **Auth:** Google-only OAuth → app-JWT (доктрина). **Тема:** завжди темна.
> **Легенда статусу екрана:** ✅ існує · 🔧 покращити · 🆕 новий.

---

## Зміст

- [0. Глобальні референси](#0-глобальні-референси)
- [A. Відвідувач (mobile + web)](#a-відвідувач-mobile--web)
- [B. Соціальна мережа](#b-соціальна-мережа)
- [C. Організатор](#c-організатор)
- [D. Door-staff / делегат](#d-door-staff--делегат)
- [E. Адмін / оператор](#e-адмін--оператор)

---

## 0. Глобальні референси

### 0.1 Стани квитка (`tickets.status`)

| Статус | Опис | Як потрапляє | Куди може перейти | UI-бейдж |
|---|---|---|---|---|
| `active` | Дійсний, не використаний | mint після оплати/RSVP | `used`, `returned`, `transferred` | 🟢 Дійсний |
| `used` | Просканований на вході | `validate` (active→used) | — (термінальний) | ⚪ Використано |
| `returned` | Повернений власником | `return_ticket` | — | 🟡 Повернено |
| `transferred` | Подарований/переданий | `transfer` (старий власник) | новий квиток у отримувача = `active` | 🔵 Подаровано |
| `refunded` | Повернено кошти (escrow) | admin refund / dispute resolved | — | 🔴 Повернення коштів |
| `frozen` | Заморожений (спір) | dispute відкрито | `active`/`refunded` | 🟠 Спір |

### 0.2 Стани escrow (`orders.escrow_status`)

| Статус | Що означає | Перехід | Хто/що рухає | Гроші |
|---|---|---|---|---|
| `HELD` | Утримано після оплати | → `CLEARING` після `ends_at` | sweep (`escrow.rs`) | у нас |
| `CLEARING` | Період охолодження | → `CLEARED` після `+ clearing_window_days` | sweep | у нас |
| `CLEARED` | Готово до виплати | → `PAID_OUT` | payout через `provider.transfer_payout()` | у нас |
| `PAID_OUT` | Виплачено організатору | термінальний | provider settlement webhook | у організатора |
| `DISPUTED`/`frozen` | Спір/заморожено | → `CLEARED` (force) або `REFUNDED` | admin рішення | заблоковано |
| `REFUNDED` | Повернено покупцю | термінальний | `provider.refund()` | у покупця |

> sweep **ніколи** не чіпає `DISPUTED`/`frozen`. Усі грошові переходи проходять єдиний seam — провайдер лише виконує дію.

### 0.3 Спільні компоненти

| Компонент | Поля/вигляд | Дії |
|---|---|---|
| `StatusBadge` | колір+текст за станом квитка | — |
| `EscrowTimeline` | HELD→CLEARING→CLEARED→PAID_OUT із датами | — |
| `EventCard` | постер, title, дата·місто, ціна/«Безкоштовно», бейдж «N друзів іде» (аватар-стек), save | tap→Деталі; save→`POST/DELETE /api/events/:id/save` |
| `provider_action` | `{kind: "test_autopay"\|"redirect"\|"none", url?}` | визначає лише крок оплати |

---

# A. Відвідувач (mobile + web)

## A.1 Auth (Google-only) 🔧
**Мета:** швидкий вхід через Google, видати app-JWT.

| ПОЛЕ | Тип | Об. | Нотатка |
|---|---|---|---|
| Кнопка «Continue with Google» | button | required | єдиний метод входу |
| Згода з умовами | text+link | required | «Продовжуючи, ви приймаєте Умови та Політику» |

| ДІЯ/КНОПКА | Що робить | API |
|---|---|---|
| Continue with Google | Google Sign-In → отримати `id_token` | `POST /api/auth/google {id_token}` → app-JWT |
| Відкрити Умови/Політику | web legal | — |

**Навігація:** вхідний екран для неавтентифікованих → новий користувач → Onboarding; наявний → Афіша.
**Зміни:** прибрати email/password (web+Android); Google-заглушку (Toast) замінити робочим SDK.

---

## A.2 Onboarding / Інтереси (3 кроки) 🔧

| Крок | ПОЛЕ | Тип | Об. | Нотатка |
|---|---|---|---|---|
| 1 | Категорії | multi-chip (`EVENT_CATEGORIES`) | required | основа «Для тебе» |
| 1 | Місто | autocomplete (`CityField`, deflt «Київ») | required | — |
| 1 | «Поблизу мене» + радіус | toggle + slider (км) | optional | геолокація |
| 2 | Дата народження | date picker | optional (рек.) | для age-gate 18+ |
| 3 | Пропозиції підписок | список users + «Підписатися» | optional | seed соц-графа |
| 3 | Дозвіл на push | system prompt | optional | — |

| ДІЯ | API |
|---|---|
| Зберегти інтереси+місто | `PUT /api/me/prefs {categories, city, near_me?, radius_km?}` |
| Зберегти DOB | `PATCH /api/me {dob}` |
| Підписатися | `POST /api/users/:id/follow` |
| Дозволити push | `POST /api/push/register {token}` |
| Пропустити крок | → Афіша |

**Навігація:** після auth для нових; пізніше частинами через Settings/Профіль.
**Зміни:** DOB у онбордингу (а не лише в Settings), щоб 18+ checkout не падав глухим 409.

---

## A.3 Афіша / Discovery 🔧

| ПОЛЕ | Тип | Об. | Нотатка |
|---|---|---|---|
| Search bar | text | optional | назва/організатор/місце |
| Сегменти | chips: Для тебе \| Усі \| Друзі йдуть \| Безкоштовні | — | дефолт «Для тебе» |
| Місто | filter chip | optional | дефолт із prefs |
| Дата | filter: Сьогодні/Завтра/Вихідні/Діапазон | optional | — |
| Категорія | multi-chip | optional | — |
| Ціна | filter: Безкоштовні / до N | optional | — |
| Сортування | Дата \| Популярність \| Поблизу | optional | — |
| Перегляд (web/mobile) | Список \| Мапа | optional | maping по `lat/lng` |
| Картка події | `EventCard` | — | див. §0.3 |

| ДІЯ | API |
|---|---|
| Пошук/фільтри (Усі) | `GET /api/events?q=&city=&category=&date_from=&date_to=&price_max=&free=&sort=&cursor=` |
| «Для тебе» | `GET /api/discover` |
| Tap картки | → Деталі події |
| Save | `POST/DELETE /api/events/:id/save` |
| Pull-to-refresh / infinite scroll | повтор з `?cursor=` |

**Навігація:** головна вкладка → Деталі події, профілі організаторів, `/c/[category]`, `/city/[slug]` (web SSR+JSON-LD).
**Зміни:** серверні фільтри (web зараз фільтрує в пам'яті); SSR/SEO на web; пагінація; friends-going у картці.

---

## A.4 Деталі події 🔧

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| Постер + категорія + age_rating | media+pills | — |
| Title, дата·місто, адреса | text + карта-прев'ю | tap адреси → Google Maps |
| Опис, lineup/розклад, галерея, відео, links | rich | Instagram/FB/Telegram |
| Соц-блок | аватар-стек друзів «іде/цікавиться» + лічильники | `GET /api/events/:id/social` |
| **Tiers (ticket tiers)** | список: назва, ціна, опис, «залишилось N», stepper | `EventDto.tiers[]` |
| Відгуки | `ReviewsSection` (avg+гістограма) | — |
| Схожі події | каруселя | — |

| ДІЯ/КНОПКА | Що робить | API |
|---|---|---|
| «Піду» / «Цікавить» toggle | RSVP як соц-об'єкт у стрічці друзів | `POST /api/events/:id/rsvp {state: going\|interested\|none}` |
| Вибір tier + кількість → «Купити» | paid → Checkout | (далі §A.5) |
| «Отримати» (free) | RSVP + mint без оплати | `POST /api/events/:id/reserve {tier_id, quantity, guest?}` |
| Save / Поділитися / В календар (ICS) | bookmark / Web Share / .ics | save: `/save` |
| Запросити друзів | пікер підписок → notification+DM | `POST /api/events/:id/invite {user_ids[]}` |
| Написати організатору | відкрити чат | `POST /api/messages` |
| Поскаржитися | report | `POST /api/reports {type:event}` |
| Follow організатора | — | `POST /api/users/:id/follow` |

**Навігація:** з Афіші/стрічки/профілю → Checkout, профіль організатора, чат, відгуки. Web: SSR-метадані (OG/Twitter/canonical) + JSON-LD Event.
**Зміни:** додати tiers+stepper+quantity; RSVP-кнопки; JSON-LD; прибрати single `event.price`.

---

## A.5 Checkout (pluggable payment + Test) 🔧

**Мета:** прозора покупка з tier, кількістю, промокодом, gift і provider-нейтральною оплатою.

| ПОЛЕ | Тип | Об. | Нотатка |
|---|---|---|---|
| Зведення | read | — | подія, tier×qty, subtotal, fee, **ВСЬОГО** (`fmtPrice`+currency) |
| Кількість | stepper | required | 1..min(left, max_per_order) |
| Промокод | text + «Застосувати» | optional | `apply_promo` |
| Спосіб оплати | radio (заповнює провайдер) | required | Test / картка / Apple-Google Pay |
| «Це подарунок» | toggle → recipient_email + message | optional | gift-at-checkout |
| Guest (web, без логіну) | email + name | conditional | magic-link claim |
| Згода з політикою повернення/escrow | checkbox | required | — |

| ДІЯ/КНОПКА | Що робить | API |
|---|---|---|
| Застосувати промокод | перерахунок | checkout з `?promo=` / тіло `{promo}` |
| «Перейти до оплати» | створює order intent | `POST /api/events/:id/checkout {tier_id, quantity, promo?, gift?, guest?}` → `OrderIntent{order_id, provider, provider_action}` |
| Виконати `provider_action` | `test_autopay` → екран підтвердж. → `POST /api/orders/:id/pay-test`; `redirect` → відкрити `url`; `none` → free | за `kind` |
| Стан оплати | processing / failed (ретрай) / success | webhook fulfills |

**Навігація:** з Деталей → після успіху → Мої квитки / E-Ticket (guest → `/t/[token]`).
**Зміни (P0):** прибрати хардкод LS URL з `orders.rs`; web прибрати безумовний `payTest()`; checkout повертає `provider_action`; mint N квитків в одному order.

---

## A.6 Мої квитки 🔧

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| Сегменти | Майбутні \| Минулі \| Повернені/Подаровані | — |
| Рядок | міні-постер, title, дата·місто, `StatusBadge`, мітка «подарунок» | poster вже в `TicketDto` |

| ДІЯ | API |
|---|---|
| Tap | → E-Ticket / `/me/tickets/[id]` |
| Фільтр сегмента | client або `?status=` |
| Pull-to-refresh | `GET /api/me/tickets` |

**Навігація:** вкладка/розділ профілю → E-Ticket.
**Зміни:** сегменти + постер; web: прибрати хардкод `₴` (вживати `fmtPrice+currency`).

---

## A.7 E-Ticket / QR 🔧

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| title, дата·місто, адреса, tier, ім'я власника | read | — |
| **QR** | rotating/short-lived token (HOTP/JWT exp) | НЕ показувати сирий токен текстом |
| Статус check-in | read | час сканування, якщо `used` |
| keep-screen-on + max brightness | system | при показі |

| ДІЯ/КНОПКА | Що робить | API |
|---|---|---|
| Подарувати | діалог email АБО @username/контакт | `POST /api/tickets/:id/transfer` |
| Повернути квиток | підтвердж. із сумою/строком | `POST /api/tickets/:id/return` |
| Відкрити спір (escrow held\|clearing) | dispute | `POST /api/orders/:id/dispute {reason}` |
| Add to Google/Apple Wallet | .pkpass / Google Wallet | — |

**Навігація:** з Мої квитки; повернення/подарунок → назад у список. Web: `/me/tickets/[id]` (owner) + підписаний `/t/[token]` (guest).
**Зміни (P0):** rotating QR; прибрати `Text(qrToken)` (`ETicketScreen.kt:98`).

---

## A.8 Подарувати квиток 🔧

| ПОЛЕ | Тип | Об. | Нотатка |
|---|---|---|---|
| Отримувач | email АБО @username/контакт | required | вибір способу |
| Повідомлення | text | optional | — |

| ДІЯ | API |
|---|---|
| Надіслати подарунок | старий квиток → `transferred`, новий у отримувача = `active` | `POST /api/tickets/:id/transfer {recipient}` |

**Навігація:** з E-Ticket та (нове) кнопка на картці у «Мої квитки».
**Зміни:** не лише email; підвищити помітність.

---

## A.9 Повернення / Refund 🔧

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| Сума повернення | read | повна якщо `HELD`; після `CLEARING` — за політикою |
| Строки | read | прив'язка до escrow |
| Причина | select/text | optional |

| ДІЯ | API |
|---|---|
| Підтвердити повернення | квиток → `returned`, escrow → `REFUNDED` через `provider.refund()` | `POST /api/tickets/:id/return` |

**Навігація:** з E-Ticket → назад у список.
**Зміни:** показувати суму/строки/політику перед діями.

---

# B. Соціальна мережа

## B.1 Стрічка (Feed) 🔧

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| Сегмент-таб | sticky chips: Для тебе \| Підписки \| Популярне | — |
| Inline-композер | avatar + «Поділіться чимось…» | відкриває повний композер |
| Картка поста | avatar-фото + verified, ім'я, kind+час+reason, текст, медіа-грід(0..4), mini-card події, дії | — |
| Соц-об'єкт RSVP | «Ім'я та ще N йдуть на ‹Подія›» + аватар-стек | — |
| Соц-об'єкт follow | «Ім'я почав(ла) стежити за ‹Організатор›» + «Підписатись» | — |

| ДІЯ | API |
|---|---|
| Pull-to-refresh / infinite scroll | `GET /api/feed?cursor=<created_at,id>&filter=for_you\|following\|popular&limit=` |
| Лайк (оптимістично) | `POST/DELETE /api/posts/:id/like` |
| Коментар | → Коментарі |
| Поділитися | bottom-sheet: Repost / DM / системний deep-link |
| «...» меню | Поскаржитись `POST /api/reports`; Заблокувати `POST /api/users/:id/block`; Прибрати з інтересів |

**Навігація:** нижня навігація «Стрічка» → Профіль автора, Деталі події, Коментарі, Композер.
**Зміни (P0):** курсор-пагінація+ранжування `score=recency*w1+(likes+comments)*w2+friends_boost`; таби; RSVP/follow як feed-об'єкти.

---

## B.2 Композер поста 🆕

| ПОЛЕ | Тип | Об. | Нотатка |
|---|---|---|---|
| Текст | multiline ≤2000 + лічильник | required* | *або медіа |
| Фото | grid 0..4 | optional | `POST /api/uploads` |
| Прикріплена подія | пікер (свої/публічні) | optional | event_share |
| Видимість | select: Публічно \| Лише підписники | required | `posts.visibility` |

| ДІЯ | API |
|---|---|
| Опублікувати | `POST /api/posts {text, media_urls[], event_id?, visibility}` |
| Скасувати | → назад |

**Навігація:** FAB у Стрічці / кнопка у Профілі.

---

## B.3 Профіль — власний 🔧

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| Аватар-фото, ім'я, @handle, bio, verified, роль-чіп | read | handle — нове |
| Лічильники | Підписники \| Підписки \| Події \| Пости | клікабельні |
| Вкладки | Пости \| Події \| Відгуки | — |

| ДІЯ | API |
|---|---|
| Tap «Підписники»/«Підписки» | → Список користувачів | `GET /api/users/:id/followers\|following` |
| Редагувати профіль | ім'я, bio, avatar upload, handle | `PATCH /api/me` + `/api/uploads` |
| Поділитися профілем | deep-link / QR-візитка | — |
| Новий пост | → Композер | — |

**Навігація:** нижня навігація «Профіль» → Settings, Interests, Followers/Following, Композер.

---

## B.4 Профіль — публічний 🔧

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| Аватар-**ФОТО**, ім'я, @handle, verified, bio | read | FIX: рендерити `avatar_url`+`verified` |
| Рейтинг організатора | зірки avg + count | якщо є |
| Лічильники | клікабельні | — |
| «Спільні друзі» | аватар-стек + N | `friends_in_common` (нове) |
| Вкладки | Пости \| Події \| Відгуки | — |

| ДІЯ | API |
|---|---|
| Підписатись/Відписатись (+чекбокс «Пуш про нові події» для організатора) | `POST/DELETE /api/users/:id/follow` |
| Написати | → Чат |
| «...» | Поділитися; Поскаржитись `POST /api/reports{type:user}`; Заблокувати `/block`; Mute `/mute` |
| Tap лічильників | → списки |

**Навігація:** з постів, коментарів, friends-going, пошуку → Чат, Деталі події, Списки.

---

## B.5 Підписки — Список користувачів 🆕

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| Рядок | avatar, ім'я, @handle, статус «Підписані на вас»/«Ви підписані» | — |
| Кнопка | Підписатись/Підписані | inline |
| Пошук у списку | text | client-фільтр |

| ДІЯ | API |
|---|---|
| Підписатись/Відписатись inline | `POST/DELETE /api/users/:id/follow` |
| Tap рядок | → Публічний профіль |

**Навігація:** з клікабельних лічильників профілю та «N лайкнули».

---

## B.6 Пост + Коментарі 🔧

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| Рядок коментаря | avatar-фото, ім'я (tap→профіль), час, текст, лайк+лічильник | — |
| Поле вводу | text ≤2000 + надіслати | — |
| Відповідь | one-level reply | `parent_id` (нове) |

| ДІЯ | API |
|---|---|
| Додати коментар | `POST /api/posts/:id/comments` |
| Лайк коментаря | `POST/DELETE /api/comments/:id/like` |
| Відповісти | `POST /api/posts/:id/comments {parent_id}` |
| «...» | видалити (свій) `DELETE /api/comments/:id`; поскаржитись |

**Навігація:** з будь-якого `PostCard`.
**Зміни:** лайки/відповіді/видалення коментарів; аватари; перехід на профіль.

---

## B.7 Відгуки 🔧

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| avg + гістограма зірок | read | — |
| Фільтр/сорт | Нові \| Корисні \| За рейтингом | — |
| Картка відгуку | avatar+ім'я, бейдж «Був(ла) на події», зірки, текст, фото, «корисно» N | verified attendee |
| Відповідь організатора | read/write | нове |

| ДІЯ | API |
|---|---|
| Залишити/Оновити | `POST /api/reviews` — **гейт: EXISTS ticket status IN(active,used) AND starts_at<now** |
| «Корисно» | `POST /api/reviews/:id/helpful` |
| Організатор: Відповісти | `POST /api/reviews/:id/reply` |
| Поскаржитись | `POST /api/reports {type:review}` |

**Навігація:** з Деталей події; автопост на стіну автора.
**Зміни (P0):** verified-attendee гейт замість лише часового.

---

## B.8 Чат / DM 🔧

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| Шапка | avatar+ім'я peer (tap→профіль), online/typing | нове |
| Бабли | мої/чужі, час, read-receipt галочки | поле `read` вже є |
| Поле вводу | text + фото + прикріпити подію/квиток | вкладення — нове |

| ДІЯ | API |
|---|---|
| Надіслати | `POST /api/messages` (WebSocket `/ws` доставка, REST fallback) |
| Прикріпити фото/подію | `/api/uploads` / пікер |
| «...» у шапці | Заблокувати, Поскаржитись, Очистити |

**Навігація:** з публічного профілю, картки події, списку діалогів.
**Зміни:** real-time замість 4с polling; read-receipt UI; медіа.

### B.8.1 Список діалогів (у Вхідні)

| ПОЛЕ | Тип | Дії/API |
|---|---|---|
| Рядок | avatar, ім'я, прев'ю «Ви: …», час, unread-badge | tap → Чат |
| Пошук | text | client |
| FAB «Новий діалог» | пошук users | → Чат |
| Свайп | Архів/Видалити | нове |

---

## B.9 Сповіщення / Inbox 🔧

### B.9.1 Сповіщення (таб)

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| Фільтр-чіпи | Усі \| Соціальні \| Події | нове |
| Картка | avatar актора (через `actor_id`), згрупований title «Ім'я та ще N…», body, час, read | групування+аватар нові |

| ДІЯ | API |
|---|---|
| Tap соц-сповіщення | → профіль актора / пост (FIX: `actor_id`, не лише event) |
| Tap подію | → Деталі |
| Mark-all-read | `POST /api/me/notifications/read` |
| Налаштування пушів за типом | → Settings |

### B.9.2 Inbox-хаб

| ПОЛЕ | Дії/API |
|---|---|
| Таби: Сповіщення \| Повідомлення з badge | перемикання |
| Глобальний badge на нижній навігації (нове) | `GET /api/me/unread` |

**Навігація:** нижня навігація «Вхідні».

---

# C. Організатор

## C.1 Онбординг / KYC 🆕

| Крок | ПОЛЕ | Тип | Об. | Нотатка |
|---|---|---|---|---|
| 1 | display_name | text | required | публічна назва |
| 1 | bio | textarea | optional | — |
| 1 | avatar | image | optional | — |
| 1 | payout_country | select | required | визначає валюту/провайдера |
| 2 (KYC, можна відкласти) | name | text | required | власник рахунку |
| 2 | iban | text | required | 2 літери+2 цифри+11–30 |
| 2 | swift | text | required | 8 або 11 симв. |

| ДІЯ | API |
|---|---|
| «Стати організатором» | `POST /api/me/organizer-profile` |
| «Надіслати на перевірку» | `POST /api/me/kyc` → `kyc_status=pending` |
| «Пропустити поки» | → /organizer |
| «Створити першу подію» | → /organizer/new |

**Навігація:** з /account та Android profile → /organizer.
**Правило:** продаж платних/free дозволено в test, але **payout вимагає `kyc_status=verified`**.

---

## C.2 Створити подію (усі поля + tiers) 🔧

| ПОЛЕ | Тип | Об. | Нотатка |
|---|---|---|---|
| title | text | required | — |
| description | textarea | optional | — |
| category | select | required | — |
| city + venue_address | autocomplete + text | required/optional | geo резолвить сервер |
| starts_at / ends_at / timezone | datetime ×2 + select | required/opt/opt | tz deflt міста |
| visibility | select: published \| unlisted \| hidden | required | unlisted=за лінком |
| age_rating | select: без/13/16/18/21 | optional | — |
| poster / gallery / video_url / links | image / multi / url / csv | optional | — |
| **TICKET TIERS** (repeatable ≥1) | блок | required | див. нижче |
| is_draft | checkbox | optional | web+Android |

**Tier (повторюваний рядок):**

| ПОЛЕ tier | Тип | Об. | Нотатка |
|---|---|---|---|
| name | text | required | GA/VIP/Early-bird |
| price | decimal | required | 0 = free/RSVP |
| currency | select | required | — |
| capacity | int | required | per-tier `sold/capacity` |
| max_per_order | int | optional (deflt 10) | — |
| sales_start_at / sales_end_at | datetime | optional | вікно продажу |
| visibility | select: public \| hidden \| locked | required | — |

| ДІЯ | API |
|---|---|
| «Додати рівень» / «Видалити рівень» | local (видалити заборонено якщо `sold>0`) |
| «Опублікувати» | `POST /api/events` + `POST /api/events/:id/tiers` (status=published) |
| «Зберегти чернетку» | status=draft |

**Навігація:** з dashboard → після збереження /organizer/[id].
**Зміни (P0):** динамічний tiers-редактор замість одного price+tickets_total; Android draft-перемикач.

---

## C.3 Редагувати / Скасувати 🔧

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| Усі поля C.2 + tiers | form | редагування tiers (видалення заборонено якщо `sold>0`) |
| sales_closed | toggle | Стоп/Відновити продаж |

| ДІЯ | API |
|---|---|
| Зберегти | `PUT /api/events/:id` + `PUT/POST/DELETE /api/tiers/:id` |
| Стоп/Відновити | `PUT /api/events/:id {sales_closed}` |
| Скасувати подію | `DELETE /api/events/:id` (confirm; тригерить refund held/clearing) |
| published↔unlisted | `PUT {visibility}` |

**Навігація:** з dashboard «Редагувати».

---

## C.4 Керування подією (продажі, гості, check-in) 🔧

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| Зведення | sold/capacity + виручка **по КОЖНОМУ tier**, checked_in | — |
| Таблиця гостей | owner_name, owner_email, tier_name, status, promo_code, bought_at, validated_at | — |
| Фільтри | за tier, статусом, пошук name/email | — |

| ДІЯ | API |
|---|---|
| Експорт CSV (з tier/promo) | `GET /api/events/:id/tickets` export |
| Надіслати квиток повторно | `POST /api/tickets/:id/resend` |
| «Контроль на вході» | → Check-in / Door Mode |
| Список гостей доступний staff з `view_list` | `GET /api/events/:id/tickets` (через `require_perm`) |

**Навігація:** з dashboard «Керувати» → Check-in, Команда, Промокоди.

### C.4.1 Check-in / Live stats

| ПОЛЕ | Дії/API |
|---|---|
| checked_in/sold (загалом+per tier), останні скани | `GET /api/events/:id/checkin-stats` |
| Кнопка Scan | → Door Mode (`POST /api/tickets/validate`) |

---

## C.5 Промокоди 🔧

| ПОЛЕ | Тип | Об. | Нотатка |
|---|---|---|---|
| code | text | required | — |
| percent_off / amount_off | number | required (один) | fixed або % |
| tier | select | optional | прив'язка до tier |
| max_uses | int | optional | — |
| expires_at | datetime | optional | — |

| ДІЯ | API |
|---|---|
| Створити код | `POST /api/events/:id/promos {code, percent_off?, amount_off?, tier_id?, max_uses?, expires_at?}` |
| Видалити | `DELETE /api/promos/:id` |

**Навігація:** з Керування подією.
**Зміни:** додати max_uses/expires_at/tier-binding/amount_off у UI.

---

## C.6 Аналітика 🔧

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| Період | 7д/30д/усе/кастом | — |
| Воронка | перегляди→checkout→оплачено (conversion %) | — |
| Revenue/tickets by tier | bar | — |
| Daily sales | line/bar | — |
| Refund/return rate, sell-through % | read | — |
| Top promo codes | таблиця | — |

| ДІЯ | API |
|---|---|
| Зміна періоду/події | `GET /api/organizer/analytics?from=&to=&event_id=` |
| Експорт CSV | export |

**Навігація:** з dashboard «Аналітика».

---

## C.7 Виплати (escrow, provider-neutral) 🔧

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| KPI | held_in_escrow, ready_for_payout (cleared), paid_out | per-currency |
| Per-event breakdown | event, held, clearing, cleared, очікувана дата clearing (`ends_at + clearing_window_days`) | — |
| Історія виплат | date, amount, currency, order_count, status, failure_reason | — |

| ДІЯ | API |
|---|---|
| «Запросити виплату» (active коли cleared>0 і kyc verified) | `POST /api/organizer/payouts/request` |
| Перехід до KYC якщо не verified | банер → onboarding KYC |

**Навігація:** з dashboard «Виплати».
**Зміни:** organizer-initiated request; per-event breakdown; provider-нейтральний текст («виплата платформою»).

---

## C.8 Команда / Door-staff (інвайти, права) 🆕

| ПОЛЕ | Тип | Об. | Нотатка |
|---|---|---|---|
| email | text | required | кого запросити |
| perms | multi-chip: `scan` / `view_list` / `refund` / `manage_staff` | required (deflt `scan`) | — |
| scope | усі події \| конкретна | required | — |
| Список staff | read | — | name/email, статус (invited/active/revoked), права, invited_by |

| ДІЯ | API |
|---|---|
| «Запросити» (owner / `manage_staff`) | `POST /api/events/:id/staff {email, perms[]}` → invite_token + notification/email |
| Перелік | `GET /api/events/:id/staff` |
| Змінити права | `PATCH` (re-issue perms) |
| Відкликати | `DELETE /api/events/:id/staff/:staff_id` (status=revoked, audit) |

**Навігація:** з dashboard «Команда» і з Керування подією.
**Модель:** делегування — це per-event **relationship** (`event_staff`), а НЕ глобальна роль. Глобальний `users.role` лишається `{USER, ADMIN}`. Єдиний seam авторизації: `staff::require_perm(db, event_id, user_id, perm)` — owner проходить завжди.

---

# D. Door-staff / делегат

## D.1 Прийняти інвайт (deep-link `/staff/invites/:token`) 🆕

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| Подія + дата | read | — |
| Перелік наданих прав | chips | read |
| Organizer name | read | — |

| ДІЯ | API |
|---|---|
| «Прийняти» | `POST /api/staff/invites/:token/accept` (bind user_id, status=active) — **token одноразовий** |
| «Відхилити» | закрити |
| (не залогінений) | спершу логін → accept |

**Навігація:** з email/push/notification → після accept → «Мої події/зміни».

---

## D.2 Мої події / «Мої зміни» 🆕

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| Список подій | title, дата, місто, права-chips | `GET /api/me/staff-events` |
| Badge «сьогодні» | для активних подій | — |

| ДІЯ | API |
|---|---|
| Tap подію | → Door Mode (scan) |
| «Список гостей» (якщо `view_list`) | → екран списку з пошуком за іменем (email прихований без `pii`) |

**Навігація:** окремий вхід для тих, хто НЕ власник (звичайний MyEvents їм нічого не показує).

---

## D.3 Скан QR + результати валідації 🔧

**Door Mode (event-scoped, безперервний, офлайн-толерантний).**

| ПОЛЕ/БЛОК | Тип | Нотатка |
|---|---|---|
| Header | event title + лічильник «N / M» | live |
| Камера | full-screen | безперервний скан-loop |
| Останні 10 сканів | час, ім'я, tier, вердикт-колір | — |
| Індикатор online/offline + к-ть у черзі | read | — |

| ДІЯ | API / Поведінка |
|---|---|
| Скан QR | `POST /api/tickets/validate {qr_token, event_id}` (auth через `require_perm('scan')`) |
| Офлайн | локальна перевірка по кешу + Room-черга → фонова синхронізація (server ідемпотентний за `qr_token,scanned_at`) |
| «Пошук за іменем» (fallback, `view_list`) | ручний check-in (active→used) |
| «Завершити зміну» | вихід |

**Вердикти валідації:**

| Вердикт | Колір | Стан | Мутація |
|---|---|---|---|
| ✓ Дійсний | зелений | active→used | так (+ `scanned_by`, scan_event) |
| ✗ Уже використаний | бурштин | used | ні (показати хто/коли впустив) |
| ✗ Не ваша подія | сірий | `ticket.event_id != req.event_id` | ні (до будь-якої мутації) |
| ✗ Повернено/Refunded | червоний | returned / refunded | ні |
| ✗ Недійсний | червоний | inactive/frozen | ні |

`ValidateResp`: `{valid, reason, owner_name, ticket_tier, validated_at, scanned_by_name}`. beep + haptic на кожен вердикт.

**Навігація:** з «Мої зміни» (staff) або «Контроль на вході» (owner).
**Зміни (P0):** event-scoped continuous Door Mode замість one-shot AlertDialog; `event_id` обов'язковий (breaking → версіонувати клієнт).

---

# E. Адмін / оператор

## E.1 Login 🔧

| ПОЛЕ | Тип | Об. |
|---|---|---|
| email | text | required |
| password | password | required |
| otp_code | text | required if MFA |

| ДІЯ | API |
|---|---|
| Sign in | `POST /api/auth/login` → `/api/me` має `role=ADMIN`, інакше очистити токен |

**Навігація:** → /dashboard; shell auth-guard на кожній сторінці.

---

## E.2 Дашборд 🔧

| ПОЛЕ/БЛОК (read, per-currency) | Нотатка |
|---|---|
| GMV today/7d/30d | — |
| Platform fee earned | — |
| **Escrow held (liability)** | гроші, які ми утримуємо |
| Cleared awaiting payout | — |
| Paid out / Refunded total | — |
| Disputed/frozen count, pending payouts count+amount, KYC pending count | — |
| GMV & fees line chart (30d) | — |

| ДІЯ | API |
|---|---|
| Завантаження | `GET /api/admin/stats` + `GET /api/admin/stats/timeseries?metric=gmv\|fees\|orders&days=` |
| Alert-чіпи з лічильниками | deep-link → `/reports?status=open`, `/orders?escrow=frozen`, `/payouts?status=pending`, `/kyc?status=pending` |

---

## E.3 Модерація — Users / Events

### E.3.1 Users 🔧

| ПОЛЕ (фільтри) | Тип |
|---|---|
| q, role, blocked, kyc_status, verified | text/select |
| Detail drawer | events organized, GMV, refund/dispute rate, escrow held, payout history, risk flags |

| ДІЯ | API |
|---|---|
| Пошук+фільтри | `GET /api/admin/users?q=&role=&blocked=&kyc_status=&verified=` |
| Detail | `GET /api/admin/users/:id` |
| Block/Verify/Role/Restrict-payouts | `PATCH /api/admin/users/:id {blocked\|verified\|role\|payout_restricted}` |

### E.3.2 Events 🔧

| ПОЛЕ | Тип |
|---|---|
| status (filter), q, detail (sold/revenue/escrow/reports) | select/text |

| ДІЯ | API |
|---|---|
| Список/detail | `GET /api/admin/events?status=&q=` · `GET /api/admin/events/:id` |
| Publish/Hide/Block | `PATCH /api/admin/events/:id {status}` |
| Orders події | → `/orders?event_id=` |

---

## E.4 Скарги (Reports) 🔧

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| status (filter) | select | open\|reviewed\|resolved\|all |
| Рядок | target_type, **target preview snapshot** (title/excerpt/text/email), reason, reporter, report count | не truncated UUID |
| note | textarea | optional |

| ДІЯ | API |
|---|---|
| Список (enriched) | `GET /api/admin/reports?status=` |
| Resolve | `PATCH /api/admin/reports/:id {status, note}` |
| Enforce: warn / block user / hide\|block event / delete post\|review / restrict-payouts | відповідні endpoints + auto-resolve |

---

## E.5 Escrow-керування (Orders) 🔧

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| status / escrow / currency (filters) | select | — |
| Detail drawer | buyer, organizer, event, amount, currency, provider, **provider_ref**, escrow_status, ticket, promo, dispute_reason+disputed_at, timeline | — |

| ДІЯ | API |
|---|---|
| Список/detail | `GET /api/admin/orders?status=&escrow=&currency=&event_id=` · `GET /api/admin/orders/:id` |
| Refund | `POST /api/admin/orders/:id/escrow {action:refund}` → **`provider.refund()` через seam**; квиток→returned, звільнити місце, відновити promo |
| Freeze | `{action:freeze}` |
| Release | `{action:release}` (held\|clearing→cleared) |
| Force-release disputed | `{action:release, force:true}` |

**Зміни (P0):** refund/release/payout мають реально викликати provider (зараз — лише DB-флоп).

---

## E.6 Виплати (Payouts) 🔧

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| status (filter) | select | pending\|processing\|paid\|failed |
| Рядок | date, organizer, amount+currency, order_count, status, provider_ref, failure_reason | — |
| Detail | exact orders included | — |
| Batch confirm | усі організатори due + total per currency | — |

| ДІЯ | API |
|---|---|
| Batch settle all due | `POST /api/admin/payouts/run` (cleared+verified, один payout/організатор/валюта) |
| Settle one | `POST /api/admin/organizers/:id/payout` |
| Mark status | `PATCH /api/admin/payouts/:id {status, failure_reason}` → під real provider тригерить `provider.transfer_payout()` + чекає settlement webhook; під TestProvider — миттєво |
| Detail | `GET /api/admin/payouts/:id` |

**Gate:** `blocked=false AND kyc_status=verified`.

---

## E.7 KYC-рев'ю 🆕

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| status / kyc_kind (filters) | select | individual\|business |
| Drawer | payout_name, IBAN (masked), SWIFT, business name+reg number (KYB), doc preview, submitted_at | — |
| decision_reason | textarea | required on reject |

| ДІЯ | API |
|---|---|
| Список | `GET /api/admin/kyc?status=&kind=` |
| Approve | `POST /api/admin/users/:id/kyc {status:verified}` |
| Reject | `POST .../kyc {status:rejected, reason}` (audited) |

**Навігація:** з dashboard KYC-чіпа; approve розблоковує payout.

---

## E.8 Рефанди

Рефанди виконуються з [E.5 Escrow-керування](#e5-escrow-керування-orders) (`action:refund`) або резолюцією спору. Кожен рефанд:
1. Викликає `provider.refund(provider_ref, amount, reason)` через seam.
2. Квиток → `refunded`/`returned`, escrow → `REFUNDED`.
3. Звільняє місце в tier, відновлює promo-uses.
4. Пише `ledger_entry` (credit) + `audit::log` у тій самій транзакції.

---

## E.9 Реконсиляція (Finance) 🆕

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| from / to | date | required |
| currency | filter | — |
| Rollup (read) | GMV, platform_fee, refunds, net_to_organizers, escrow_opening/closing, paid_out, provider_fees | — |
| Ledger table | append-only double-entry | `ledger_entries` |

| ДІЯ | API |
|---|---|
| Rollup | `GET /api/admin/reconciliation?from=&to=&currency=` |
| Export | `GET /api/admin/export/{orders\|payouts\|refunds\|ledger}.csv?from=&to=` |

---

## E.10 Audit / Journal 🔧

| ПОЛЕ (filters) | Тип |
|---|---|
| admin_id, action, target_type, from/to | select/date |
| Рядок | time, moderator, human-readable action, target (deep-link), formatted detail |

| ДІЯ | API |
|---|---|
| Запит | `GET /api/admin/actions?admin_id=&action=&target_type=&from=&to=` |
| Export | `GET /api/admin/export/audit.csv` |

**Контракт:** `audit::log` пишеться в тій самій транзакції, що й мутація (atomic). Усі нові money/moderation дії наслідують цей патерн.

---

## E.11 Payment Settings (provider-agnostic) 🔧

| ПОЛЕ | Тип | Нотатка |
|---|---|---|
| mode | select test\|live | — |
| provider | select noop\|test\|lemonsqueezy\|stripe\|… | з capability descriptor |
| platform_fee_bps | number | provider-neutral |
| clearing_window_days | number | provider-neutral |
| Dynamic provider creds | rendered fields | write-only, `*_set` boolean |
| Readiness checklist | read | per provider |

| ДІЯ | API |
|---|---|
| Capabilities | `GET /api/admin/payment-providers` |
| Get/Save | `GET/PUT /api/admin/payment-settings` (live вимагає всі creds) |
| Readiness | `GET /api/admin/payment-readiness` |
| Generic webhook | `POST /api/webhooks/:provider` → `provider.verify_webhook()` |

---

## Архітектурний seam (для всіх грошових екранів)

```
trait PaymentProvider {
  async fn create_charge(ChargeReq) -> ChargeResult{provider_ref, checkout_url?, status}
  async fn hold(provider_ref) / release(provider_ref)
  async fn transfer_payout(PayoutReq) -> TransferResult
  async fn refund(RefundReq) -> RefundResult
  fn verify_webhook(headers, raw_body) -> WebhookEvent{kind, order_ref|payout_ref, provider_ref}
}
impl: NoopProvider (free/RSVP), TestProvider (admin mark-as-paid), LemonSqueezyProvider, …
```

**Інваріанти:** домен `tickets`/`orders`/`escrow` викликає **лише** трейт; жоден product-екран не знає провайдера; `checkout()` повертає `provider_action`; escrow sweep провайдер-нейтральний; провайдер обирається через `payment_settings.provider`.

---

**P0 для запуску продукту вже зараз:** PaymentProvider trait + Noop/Test/LS impls + `/api/webhooks/:provider`; ticket_tiers (модель+UI); event_staff делегування + `require_perm`; Google-only auth; rotating QR; free/RSVP + admin mark-as-paid; курсор-пагінація+ранжування стрічки + RSVP-feed-об'єкти; verified-attendee гейт відгуків; block/mute.