# Underground — продуктовий аудит і роадмап до production-ready (2026-06-14)

Бачення: зручна, красива платформа пошуку й організації івентів, що зводить **криейторів і публіку**
через **соц-мережеву механіку** + **алгоритми предложки**, **територіально — весь світ**, і допомагає людям
**відкрити те, чого вони б не знайшли деінде**. Головне: зручність, дизайн, логічна соц-складова, що
**працює на поширення нових івентів**.

---
## Частина 1. Аудит (Product Designer + Product Manager)

Легенда зрілості: 🟢 готово · 🟡 частково · 🔴 прогалина (vs бачення). Severity: P0 критично для бачення.

### 1.1 Discovery & «предложка» (ядро бачення) 🔴 **P0**
- Є: `GET /api/feed` = пости підписок ∪ пости про події з категорій інтересу. Афіша = плоский каталог
  з фільтрами. Сортування лише за датою/ціною.
- Нема: **ранжування під користувача** (релевантність, трендовість, соц-докази, близькість), стрічки
  «Для тебе», «схожі події», «зараз популярне», explore нових категорій. Тобто головного — алгоритму,
  що «відкриває нове» — фактично немає. Це найбільша прогалина.

### 1.2 Локація / worldwide 🔴 **P0**
- Є: `city` — вільний текст; `country/lat/lng` у БД є, але не використовуються; адреса→Google Maps.
- Нема: вибір країни/міста, **«поряд зі мною»**, відстань, мапа подій, гео-ранжування. «Весь світ»
  поки декларативний (UA/UAH хардкод). Без локації предложка не може бути територіальною.

### 1.3 Соц-механіка поширення (віральність) 🟡 **P0**
- Є: підписки (на будь-кого), стіна, автопости (анонс події + репост відгуку), лайки, коменти, чат,
  публічні профілі з лічильниками.
- Нема: **«Зберегти/Цікавить»** (зірочка зараз мертва), **«Піду»** як соц-сигнал, **шер/репост чужої події**,
  **сповіщення підписникам, коли той, на кого ти підписаний, оголошує подію** (головний віральний цикл!),
  згадки, «3 друзі цікавляться», запрошення друзів. Петлі поширення слабкі.

### 1.4 Онбординг і персоналізація 🟡
- Є: вибір інтересів + місто (редаговано пізніше).
- Нема: дозвіл на локацію, **пропозиції кого зафоловити** (cold-start), приклади подій, пояснення цінності.

### 1.5 Сторінка події 🟡
- Є: медіа, мапа, відгуки (після початку), купівля, лінк на організатора, написати організатору.
- Нема: соц-докази (**N піде / M цікавиться**, хто з друзів), **схожі події**, **поділитися**, додати в
  календар, інші події організатора, кількість квитків/тарифи.

### 1.6 Інструменти організатора 🟡
- Є: створення/редагування/керування (web+mobile), скан QR, кабінет із статистикою, відвідувачі+CSV.
- Нема: глибша аналітика (джерела трафіку, конверсія), промокоди/знижки, тарифи квитків, чернетки,
  повторювані події, верифікація організатора.

### 1.7 Монетизація / ескроу 🟡 (відомо)
- Є: test-режим, стани ескроу, адмін-дії (refund/freeze/release), комісія.
- Нема: live Lemon Squeezy (webhook), авто-кліринг, виплати організаторам, UX повернень.

### 1.8 Сповіщення 🟡
- Є: in-app за prefs-матчем + unread-лічильники.
- Нема: **push (FCM)**, соц-сповіщення (тебе лайкнули/прокоментували/зафоловили), нагадування перед подією,
  follower-new-event.

### 1.9 Trust & Safety 🟡
- Є: адмін-модерація (published/hidden/blocked), блок юзерів.
- Нема: **скарги на подію/контент**, верифіковані організатори, анти-фрод, вікові обмеження на UI.

### 1.10 i18n / мульти-валюта (для «весь світ») 🔴
- Хардкод укр + UAH. Для глобальності — локалі, валюти, формати дат/чисел.

### 1.11 Web SEO / продуктивність 🟡
- Веб клієнт-рендерений (CSR) → погано для SEO/розшарювання (а discovery-платформі органіка важлива).
  Потрібен SSR/метатеги/OpenGraph для сторінок подій.

### 1.12 Дизайн-система 🟡
- Темна тема, частина компонентів уніфікована; звірки з Figma (44 фрейми) ще не було (TASK-014-F).

---
## Частина 2. Початковий план (PM): задачі по прогалинах
(Зведено в епіки; деталі — у фінальному роадмапі нижче.)
E1 Discovery-двигун · E2 Локація/гео · E3 Соц-віральність · E4 Сторінка події соц-докази ·
E5 Онбординг cold-start · E6 Push+соц-сповіщення · E7 i18n+валюти · E8 Trust&Safety ·
E9 Web SSR/SEO · E10 Організатор-аналітика/тарифи · E11 Live-платежі/виплати · E12 Дизайн-звірка Figma.

---
## Частина 3. Рефайн (Product Analyst): метрики, воронка, віральність, перепріоритезація

**Північна зірка:** к-сть **успішних відкриттів** = квиток/збереження на подію, яку юзер не шукав явно
(прийшов із предложки/соц-поширення), на тиждень.

**Воронка активації:** signup → онбординг(інтереси+гео) → перша дія в предложці (save/відкрив деталі)
→ перший фоловінг → перший квиток. Зараз «дірки»: після онбордингу нема куди вести (предложки нема),
збереження не існує (втрата найдешевшого сигналу інтересу), cold-start без фоловів.

**Віральність (K-factor):** головні цикли, яких бракує і які **дешеві**:
1. *Follower-notify*: підписаний бачить, що організатор оголосив подію → приходить. (дуже дешево, велика дія)
2. *Save/Interested* як публічний сигнал → «3 друзі цікавляться» на картці → соц-доказ → клік.
3. *Share* події у стрічку/назовні. (середньо)

**Перепріоритезація (дешево×важливо першим):**
- 🥇 **M1**: `save/interested` (дешево, годує і алгоритм, і соц-доказ) + **follower-notify** (дешевий
  віральний цикл) + **ранжований `/api/discover` «Для тебе»** (ядро). Це закриває E1+частину E3 і
  миттєво наповнює предложку та петлі поширення — максимальний ефект на північну зірку.
- 🥈 **M2 Локація worldwide**: country+гео, «поряд», гео-буст у предложці. Робить discovery
  територіальним (умова бачення).
- 🥉 **M3 Сторінка-події соц-докази + share + схожі + календар**: підсилює конверсію і поширення.
- **M4** Push (FCM) + соц-сповіщення + нагадування. **M5** i18n+валюти. **M6** Live-платежі+виплати+T&S.
  **M7** Web SSR/SEO. **M8** Організатор-аналітика/тарифи/промо. **M9** Дизайн-звірка Figma + полір.

Аналітик-нотатка: інструментувати події (`signup, onboarding_done, discover_impression, save, ticket,
follow, share, follower_notify_open`) — без цього не виміряти північну зірку. Винесено в M4 (разом зі
сповіщеннями) як легку телеметрію; поки лог-рівень.

---
## Частина 4. Фінальний роадмап

| M | Назва | Зміст | Стан |
|---|---|---|---|
| **M1** | **Discovery & поширення** | save/interested, `/api/discover` «Для тебе» (інтереси+місто+підписки+тренд+recency, reason), follower-notify на новій події; сурфейс mobile+web | **цей реліз** |
| **M2** | **Локація worldwide** | газетір міст (світ), геокод-на-створенні, country у prefs, гео-буст «поряд» (haversine) у discover, вибір міста (автокомпліт), відстань на картках | **DONE** |
| **M3** | **Подія: соц-докази** | N піде/M цікавиться + друзі, схожі події, share, add-to-calendar | **DONE** |
| **M4** | **Сповіщення** | FCM push (плумбінг+ресівер), соц-сповіщення (лайк/коммент/фолов), нагадування перед подією, телеметрія | **DONE** |
| **M5** | **i18n + валюти** | мульти-валюта (country→ISO, символи) на всіх поверхнях; web UA/EN; локаль-залежні сповіщення | **DONE** |
| **M6** | **Live-платежі+T&S** | LS webhook (HMAC), escrow-кліринг (worker), виплати організаторам, диспути, верифікація | **DONE** |
| **M7** | **Web SSR/SEO** | SSR сторінок подій, динамічні OG-метатеги, sitemap.xml, robots.txt | **DONE** |
| **M8** | **Організатор+** | промокоди, чернетки подій, аналітика продажів (тарифи квитків — відкладено) | **DONE*** |
| M9 | Дизайн-звірка | Figma 44 фрейми, токени, полір | **BLOCKED** (Figma MCP — лише свіжа сесія; план: `docs/backlog/TASK-M9-figma-alignment.md`) |

### M1 — деталізація (реалізується зараз)
**Backend:** міграція `event_saves`(user,event) + `events.country`; `POST/DELETE /api/events/:id/save`
(+`saved_by_me`,`saves_count` на деталях); `GET /api/discover` — score = 3·інтерес + 2·місто +
4·підписка-на-організатора + тренд(продажі+збереження+відгуки, з recency); виключає минулі/власні;
`reason∈{followed,interest,city,trending}`. На `create_event` — `notify_followers` (kind
`organizer_event`), а prefs-`event_match` більше не дублюється фоловерам. Тести, lines 100%.
**Mobile:** Афіша-тогл «Для тебе»(discover)/«Усі»; робоча зірочка save на картках+деталях; reason-чип.
**Web:** секція «Для тебе» на home для залогінених; save-зірочка.

Статус: **M1 — DONE (2026-06-14).** Backend: міграція 0014 (event_saves+country), `/api/discover`
(ранжування інтереси+місто+підписки+тренд+recency, reason∈followed/interest/city/trending),
`/api/events/:id/save`+`/api/me/saves`, follower-notify (kind `organizer_event`, без дублю з event_match).
128 тестів, lines+functions 100%, clippy чисто. Mobile: Афіша-тогл «Для тебе»/«Усі», робоча
закладка-збереження на картках+деталях, reason-чипи, «N цікавляться». Web: home «Для тебе»/«Усі»
+ save на картках/деталях. Верифіковано на moto g05 (скрін: чипи «★ ВИ ПІДПИСАНІ», закладки, «1 цікавляться»).
**Далі: M2 (локація worldwide) — наступний реліз.**

### M2 — DONE (2026-06-14)
Backend: міграція 0015 (`cities` газетір ~70 міст світу + `user_prefs.country`); `cities::resolve` (місто→країна+координати,
без зовнішнього геокодера); insert/update event авто-геокодяться з газетіру (country/lat/lng); `GET /api/cities?q=`
автокомпліт; discover — +same-country буст +distance-буст (haversine, viewer-city→event), reason `near` (<100км),
поле `distance_km`; prefs auto-derive country. Mobile: `CityField` автокомпліт у створенні/онбордингу/edit,
картки показують «місто · поряд/~N км». Web: `CityInput` (datalist) у EventForm, відстань на картках.
**133 тести, lines+functions 100%, clippy/fmt чисто.** Верифіковано: discover з distance+country; backfill гео
для наявних подій. **Далі: M3 (соц-докази на події + share + схожі + календар).**

### M3 — DONE (2026-06-13)
Backend: `src/eventsocial.rs` — `GET /api/events/:id/social` (`interested_count` = saves, `going_count` =
видані квитки; `friends_interested`/`friends_going` = ті, на кого підписаний viewer, через JOIN follows;
анонім бачить лічильники, але порожні списки друзів) та `GET /api/events/:id/similar` (WITH src —
категорія+місто; ранжування «та сама категорія» вище за «те саме місто», виключає себе/минулі/інші).
Перевикористовує `social::UserBrief`. **136 тестів, lines+functions 100%, clippy/fmt чисто.**
Mobile: на деталях події — рядок соц-доказів «🔥 N цікавляться · M піде» + аватари друзів, секція
«Схожі події» (LazyRow), кнопки «Поділитися» (Intent.ACTION_SEND) і «В календар» (CalendarContract);
`EventExtrasViewModel` тягне getEvent/social/similar. Web: на `/events/[id]` — блок соц-доказів з
друзями, share через `navigator.share`/clipboard, add-to-calendar (ICS-завантаження), грід «Схожі події».
Верифіковано на живому контейнері: `social {interested_count, going_count, friends_*}`, `similar` ≥1.
**Далі: M4 (FCM push + соц-сповіщення + нагадування + телеметрія).**

### M4 — DONE (2026-06-13)
Міграція 0016: `device_tokens`, `event_reminders` (dedup-ledger), `telemetry_events`,
`notifications.actor_id`+`pushed_at`. **Соц-сповіщення** — `notifications::notify_actor`
(skip self), емітиться з `posts::like`/`create_comment` (тільки fresh, автор тягнеться наперед —
не рейситься з видаленням) і `social::follow` (kind `post_like`/`post_comment`/`new_follower`);
падають у ту саму таблицю → existing unread-бейдж/Вхідні їх показують. **Нагадування** —
`reminders::sweep(db, 24h)` фанаут `event_reminder` усім going (квитки) ∪ interested (saves) подій,
що скоро стартують, dedup через `event_reminders`; крутиться у воркері `main.rs` (поза coverage).
**Push** — `POST/DELETE /api/me/devices`; аутбокс `push::pending`/`mark_pushed` (LIMIT по
**нотифікаціях**, фанаут на всі девайси — не губить доставку на краю батча); `FcmConfig::from_env`
гейтить воркер; реальний FCM-HTTP-транспорт лишається задокументованим стабом (без `reqwest` — диск)
у `main.rs`. Android: `firebase-messaging` (gated на `google-services.json`), `UndergroundMessagingService`,
`POST_NOTIFICATIONS` дозвіл, токен → `/api/me/devices` на логіні; **білд зелений без google-services.json**.
**Телеметрія** — `telemetry::log` (best-effort, логує помилку), інструментовано signup/save/follow/ticket;
`POST /api/telemetry` (клієнт: discover_impression/share); `GET /api/admin/telemetry` (7-дн агрегат) +
панель «Телеметрія» в admin/. **150 тестів, lines+functions 100%, clippy/fmt чисто.**
Пройдено adversarial-рев'ю (24 агенти, 4 виміри): 5 реальних знахідок виправлено (push LIMIT-truncation,
2× like/comment delete-race→500, telemetry swallow-without-log). Гайд: `docs/push-fcm-setup.md`.
**Далі: M5 (i18n + мульти-валюта).**

### M5 — DONE (2026-06-13)
**Мульти-валюта:** `src/currency.rs` — `for_country` (усі 44 країни газетіру → ISO 4217: Єврозона, не-євро ЄС
PLN/CZK/HUF/SEK/DKK/NOK/RON/BGN, GBP, CHF, USD/CAD/MXN/BRL/ARS, JPY/KRW/CNY, та ін.), `symbol(code)`,
`is_supported`; **дефолт-фолбек = UAH** (не USD), щоб події з нерозпізнаним містом не регресували (резолвні
міжнародні міста все одно отримують свою валюту — Берлін→EUR). Подія на створенні авто-проставляє валюту з
країни міста; create+update валідують валюту проти allowlist (422); `currency` додано в UpdateEventReq+UPDATE.
**i18n локаль:** міграція 0017 `user_prefs.locale TEXT NOT NULL DEFAULT 'uk'`; prefs +locale (allowlist [uk,en],
норм-фолбек uk); **усі серверні сповіщення локалізуються за локаллю отримувача** через SQL CASE — event_match,
organizer_event (LEFT JOIN prefs фоловера), social like/comment/follow (`notify_actor` бере `(uk,en)`-суфікс +
CASE на локаль таргета), event_reminder body. **Web — повна UA/EN двомовність:** `lib/i18n.tsx` (LangProvider+
useLang+`tr(lang,uk,en)`+`fmtDateL`+LanguageSwitcher), co-located переклади у 10 файлах (~96 рядків), валютні
символи у `fmtPrice` (`currencySymbol`), перемикач мови в TopNav, `syncUserLocale` синкає вибір у prefs (щоб
сповіщення збігались з UI). **Android:** `Format.currencySymbol`+мульти-валютний `price` (Берлін→«650 €»),
прибрано хардкод «(UAH)» з лейблів ціни. **157 тестів, lines+functions 100%, clippy/fmt чисто, web tsc+build
чисто, Android assembleDebug чисто.** Локалізація всього Android-UI (≈226 рядків) — задокументований follow-up.
**Далі: M6 (Live-платежі LS + кліринг + виплати + T&S).**

### M6 — DONE (2026-06-13)
Міграція 0018 (`payment_settings`+`ls_webhook_secret`/`clearing_window_days`, `users.verified`, `payouts`-журнал) + деп `hmac`/`sha2`/`hex`.
**LS-вебхук** (`webhooks.rs`, public): `POST /api/webhooks/lemonsqueezy` — верифікує `X-Signature` HMAC-SHA256
над сирим тілом (constant-time, **до** парсингу); на `order_paid` мапить `meta.custom_data.order_id`→`fulfill`
(мінт квитка+escrow held, **ідемпотентно**); невалідний/відсутній підпис→401, секрет не налаштовано→409, чужі
події→200-ack. `checkout` у live-режимі віддає LS hosted-checkout URL з order_id у custom data.
**Escrow-кліринг** (`escrow.rs::sweep`): held→clearing (подія завершилась) →cleared (минув `clearing_window_days`,
дефолт 5); frozen/disputed не чіпаються; hourly-worker у `main.rs`. **Виплати** (`payouts.rs`):
`POST /api/admin/organizers/:id/payout` — усі cleared-кошти організатора (за вирахуванням комісії,
`sum·(10000-bps)/10000`, по одному запису на валюту) →orders `paid_out` + payout-журнал; `GET /api/admin/payouts`;
гроші доходять до організатора ТІЛЬКИ через явну адмін-виплату після клірингу. **T&S:** `POST /api/orders/:id/dispute`
(покупець заморожує escrow held|clearing→frozen); верифікація організатора (`users.verified`, admin patch, бейдж на
профілі+адмінці). Web: кабінет +«Виплачено»; admin: сторінка «Виплати»+верифік-тогл. **166 тестів, lines+functions
100%, clippy/fmt чисто, web/admin tsc чисто.** Adversarial-рев'ю (2 виміри×verify): з 13 знахідок 1 реальна
виправлена — **return_ticket рефанд лише з 'held'**: повернення квитка під час 'clearing'/'cleared' давало мовчазний
no-op → організатор отримував виплату за повернений квиток; тепер рефанд з held|clearing, а після cleared/paid_out
повернення відхиляється (вікно минуло). Live-смоук: підписаний вебхук→квиток (валідний 200/форджений 401), release→payout 95.00 (net 5% fee), paid_out. Гайд: `docs/payments-lemonsqueezy-setup.md`.
**Далі: M7 (Web SSR/SEO — SSR сторінок подій, OG-метатеги, sitemap).**

### M7 — DONE (2026-06-14)
Сторінку події `web/app/events/[eventId]` розділено на **server-компонент** (`page.tsx`: `generateMetadata`
тягне подію server-side → динамічні OG/Twitter-теги — title/description/url/image(постер)/site_name + canonical;
дефолтний експорт SSR-фетчить подію і рендерить клієнтський `EventDetailClient` з `initialEvent` — без флешу,
краулери бачать контент) + **client-компонент** `EventDetailClient.tsx` (вся інтерактивність). `app/sitemap.ts`
(головна + усі published-події з lastmod/changefreq/priority, revalidate 1h), `app/robots.ts` (public allow,
/me//organizer//login/register disallow + лінк на sitemap), layout `metadataBase`+OG-дефолти. **tsc+next build
чисто** (подія тепер `ƒ` server-rendered). Live-смоук: OG-теги з постером у HTML, тайтл у SSR-body, robots/sitemap
віддаються. **Далі: M8 (організатор+: промокоди, тарифи квитків, чернетки, аналітика).**

### M8 — DONE* (2026-06-13)
Міграція 0019: `promo_codes` (event-scoped, unique lower(code) per event, percent_off/max_uses/expires_at/active)
+ `orders.promo_code`. **Промокоди** (`promos.rs`): owner `POST/GET /api/events/:id/promos`, `DELETE /api/promos/:id`
(owner-only через JOIN events); checkout приймає `?promo=CODE` → `apply_promo` **атомарний** check-and-increment
(`UPDATE … WHERE used_count < max_uses RETURNING percent_off` — без TOCTOU-гонки), знижка у order.amount.
**Чернетки:** `CreateEventReq.status` (draft|published), draft прихований з афіші/деталей/checkout, **не анонсується**;
публікація (PUT status=published) анонсує один раз (announce винесено в helper; **update не дозволяє вертати в draft**
— інакше можна було б повторно спамити фоловерів). **Аналітика:** `GET /api/organizer/analytics` — продажі по днях
(30 дн, paid). Web: чекбокс «чернетка» при створенні, керування промокодами на сторінці події, типи аналітики.
**169 тестів, lines+functions 100%, clippy/fmt чисто, web tsc+build чисто.** Adversarial-рев'ю (2 виміри×verify,
12 агентів): з 10 знахідок **2 реальні виправлено** — (1) promo max_uses TOCTOU-гонка (SELECT-потім-UPDATE →
атомарний conditional UPDATE), (2) re-announce-спам через published→draft→published (заборонено вертати в draft).
Live-смоук: Berlin-checkout `?promo=SAVE20`→знижка. **\*Тарифи квитків (ticket tiers) свідомо відкладено** — це
рефакторинг ядра ціноутворення/checkout/escrow (кожен order = одна ціна/подія), що ризикує M6-платіжним ядром;
заслуговує окремого фокус-етапу. **Далі: M9 (Figma-звірка дизайну).**
