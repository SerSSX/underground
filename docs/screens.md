# Underground — Screens & Flows (page-by-page)

> Концепція: соцмережа-маркетплейс андеграундних івентів. Будь-хто створює подію й шерить її;
> платформа за **інтересами + містом/геолокацією** сповіщає релевантних людей (пуш + стрічка);
> люди відкривають, купують квитки, перепродають/дарують/повертають, лишають відгуки, підписуються
> на івентмейкерів. Івентмейкер створює подію, продає квитки, **сканує QR на вході**. Гроші — в ескроу
> до проведення події. Це й «майданчик-старт» для нових авторів, і двигун відкриття нового.
> Деталі бізнес-правил — `docs/spec.md`; реалізація — `docs/specks/SDD-*`.

## Category taxonomy (broad — від концертів до будь-чого)

Канонічний список (string `category` у БД; згруповано для кольорів/іконок):

- **Music:** Concert, Rock, Metal, Electronic, Hip-Hop, Jazz, Pop, Folk, Classical, DJ Set, Festival, Karaoke
- **Stage:** Stand-Up, Theatre, Improv, Dance, Poetry, Cinema
- **Art & Culture:** Exhibition, Photography, Fashion, Literature, Craft
- **Knowledge & Business:** Lecture, Conference, Workshop, Course, Networking, Startup, Hackathon
- **Active & Outdoor:** Sport, Yoga, Hike, Run, Cycling, Extreme
- **Community & Lifestyle:** Party, Meetup, Charity, Family, Food & Drink, Tasting, Board Games, Quiz, Market, Gaming, Spiritual, Other

Колір/іконка призначаються **за групою** (Music — червоно-фіолетова гама, Knowledge — синя, Active — зелена,
Lifestyle — бурштинова, тощо); невідома категорія → нейтральний колір + `Other`.

---

## 0. Auth / Onboarding

**S-01 Welcome / Auth** — лого UNDERGROUND, «Concerts · Party · Events».
- Елементи: «Увійти через Google»; перемикач **Реєстрація/Вхід** з email+паролем; згода з умовами; помилки валідації.
- API: `POST /api/auth/google` · `POST /api/auth/register` · `POST /api/auth/login` → app-JWT.
- Далі: новий користувач → S-02; наявний → Feed.

**S-02 Interests onboarding** — «Що вам цікаво?»: мультивибір **категорій** (таксономія вище) + **місто** (для гео-релевантності).
- API: `PUT /api/me/prefs` (`music_genres`/`event_types`→categories[], `city`).

---

## 1. Bottom navigation (5 табів)

`Стрічка` · `Пошук/Афіша` · `Квитки` (зірка) · `Сповіщення` · `Профіль`. Тема завжди темна.

---

## 2. Feed (Стрічка)

**S-10 Feed** — персональна стрічка: пости івентмейкерів, на яких підписаний, + рекомендовані події поблизу за інтересами.
- Елементи: композер «Що нового?» (свій пост: текст/фото); картки постів (автор, час, текст, фото, лайки, коментарі);
  вкраплені картки подій (рекомендації «бо вам подобається Rock у Києві»).
- Взаємодія: лайк, коментар, відкрити подію/профіль, опублікувати пост.
- API: `GET /api/feed` · `POST /api/posts` · `POST /api/posts/{id}/like|comment`.
- Стани: порожньо («Підпишись на авторів / обери інтереси»), завантаження, помилка.

**S-11 Post detail** — пост + список коментарів + поле коментаря.

---

## 3. Afisha / Search (Пошук) — двигун відкриття

**S-20 Afisha** — сітка/список подій.
- Елементи: пошук за назвою; **чіпи категорій**; фільтри (місто, дата, ціна, безкоштовні); сортування (дата/популярність/поблизу).
- Картка події: постер/градієнт, категорія, «N left», дата, назва, місце, ціна, зірка (інтерес).
- API: `GET /api/events?q&city&category&date_from&date_to&price_max&free&sort`.

**S-21 Event detail** — повна сторінка події.
- Елементи: постер; назва; **організатор** (аватар, ім'я → профіль); категорія·місто; дата/час; адреса + мапа;
  опис; ціна + залишок; **Купити квиток**; **Цікаво** (зберегти); поділитися; «Хто зацікавився»; відгуки/рейтинг; «⋮» (поскаржитися).
- API: `GET /api/events/{id}` · `POST /api/events/{id}/interest` · `POST /api/complaints`.

**S-22 Buy flow** — степер кількості → `POST /api/events/{id}/checkout` → LS hosted checkout (Custom Tab) → підтвердження → e-квиток.

---

## 4. Tickets / Saved (Зірка) — таби: Квитки / Цікаво / Мої події

**S-30 My Tickets** — куплені квитки (подія, дата, статус). Тап → e-квиток.
**S-31 E-ticket** — **QR-код**, дані події, свайп «1/N» між квитками; «⋮»: подарувати / продати / повернути.
- API: `GET /api/me/tickets` · `POST /api/tickets/{id}/transfer|sell|return`.
**S-32 Interested** — збережені події (зірка).
**S-33 My Events** — створені тобою події (роль організатора); порожній стан із «Створити».
- Кожна: «Хто купив», **Сканувати QR**, Редагувати, Зупинити продаж, Видалити.

---

## 5. Notifications (Сповіщення)

**S-40 Notifications** — гео+інтерес-матчі («Новий Rock-івент поблизу»), нагадування про квитки,
соц-події (новий підписник, коментар), апдейти організаторів, статуси ескроу/виплат.
- Тап → відповідний екран. Пуш (FCM) дублює ключові події.
- API: `GET /api/me/notifications`; матчинг за `prefs.categories` + `city`/гео.

---

## 6. Profile

**S-50 My Profile** — аватар, кавер, ім'я, біо; лічильники (підписки/підписники/публікації); таби **Пости / Підписки / Події**; редагувати; вхід у Налаштування.
**S-51 Organizer/Other Profile** — те саме + **Підписатись/Відписатись**, написати, оцінити, заблокувати, поскаржитися; таби Пости / Інтереси / Події.
- API: `GET /api/users/{id}` · `POST/DELETE /api/users/{id}/follow` · `POST /api/ratings`.
**S-52 Edit Profile** — ім'я, біо, аватар, кавер, місто.
**S-53 Settings** — тема (темна, зафіксовано), **Фінанси** (організатор), **Статистика**, Конфіденційність, Безпека, Акаунти, Вихід.

---

## 7. Event-maker (організатор)

**S-60 Create/Edit Event** — назва, опис, **категорія** (пікер із таксономії), місто, адреса (вибір на мапі), дата/час,
постер (завантажити), вартість (платно/безкоштовно) + ціна + кількість квитків; **Прев'ю**; **Опублікувати**.
- API: `POST/PATCH /api/events`.
**S-61 Event Sales** — хто купив, виторг, стан ескроу, зупинити продаж, видалити.
**S-62 QR Scanner** — камера; скан QR квитка → валідація (дійсний / вже використаний / чужий) на вході.
- API: `POST /api/tickets/{id}/validate` (або `POST /api/scan` з токеном QR).

---

## 8. Social / Messaging

**S-70 Conversations** — список діалогів (аватар, ім'я, прев'ю, час, непрочитані).
**S-71 Chat 1:1** — бульбашки, поле вводу, онлайн-статус.
- API: `GET/POST /api/conversations` · `GET/POST /api/conversations/{id}/messages` (v1 — поллінг).

---

## 9. Finance & Stats (організатор)

**S-80 Finance** — баланси за станами ескроу (HELD / CLEARING / CLEARED / виплачено); запит виплати; історія транзакцій.
- API: `GET /api/me/escrow` · (виплати ініціює адмін, BR-001).
**S-81 Statistics** — перегляди, продажі, відвідуваність (скани) по кожній події.

---

## 10. Admin (web, окремо)

Дашборд (users/events/GMV/commission), Users (block/role), Events (модерація), Complaints (черга),
**Escrow/Disputes** (uphold/reject, release/refund/extend), Payouts (settle). Деталі — `docs/spec.md` UC-011.

---

## Flow highlights

- **Discovery loop:** prefs(категорії+місто) → Notifications/Feed підкидають релевантне → Event detail → Buy → E-ticket.
- **Maker loop:** Create event → share → продажі (ескроу HELD) → QR-скан на вході → подія минула → CLEARED → виплата.
- **Resale loop:** My Tickets → продати/подарувати/повернути (refund-гілка ескроу).
- **Social loop:** підписка на мейкерів → їхні пости/події у стрічці → відгуки/рейтинг → довіра.
