# Концепція продукту та поточний стан — Underground

> Документ #1 серії. Версія: 2026-06-29. Статус кодової бази — pre-launch (працюючий каркас, не комерційно запущений).
> Джерела істини: `docs/spec.md`, `docs/rules.md` (v0.5.0, STRICT), кодова база `api/`, `android/`, `web/`, `admin/`.

---

## 1. Резюме (Executive Summary)

**Underground** — це Kyiv-first, mobile-native (Android-first) платформа, що зрощує три речі, які на українському ринку досі живуть окремо:

1. **персоналізоване discovery подій** (афіша «Для тебе» з гео- і соц-зважуванням);
2. **квитковий сервіс** із e-ticket / QR-валідацією на вході;
3. **соціальний шар** (профілі, підписки, стрічка активності, пости, коментарі, відгуки, DM-чат).

**Стрижнева відмінність** — escrow-маркетплейс: покупець платить, гроші **утримуються** (`HELD → CLEARING → CLEARED → PAID_OUT`, гілки `DISPUTED` / `REFUNDED` через єдиний seam `api/src/escrow.rs`) і виплачуються організатору **лише після того, як подія відбулась**. Це перетворює головний біль UA-ринку (неповоротний ~10% сервісний збір Karabas / Concert.ua і повільні повернення «за листом організатора») на trust-фічу: *«платиш безпечно — організатор отримує гроші лише після події; скасування = автоматичне повне повернення»*.

**Для кого:**
- **Відвідувачі** — знаходять, купують, дарують і повертають квитки; бачать, хто з друзів іде; підписуються на організаторів.
- **Організатори** — створюють події будь-якого масштабу, продають квитки, керують через кабінет, бачать аналітику й виплати.
- **Персонал / делегати** — door-staff, що сканує квитки на вході (наразі **не реалізовано** — критичний розрив).
- **Платформа-оператор** — модерує, керує ескроу, виплатами, спорами, KYC через окрему admin-панель.

**Чесна оцінка стану.** Це **рідкісно повний для pre-launch каркас**: повна escrow-стейт-машина з фоновим worker-sweep, ідемпотентний Lemon Squeezy webhook з HMAC-перевіркою, refund/dispute/gift флоу, персоналізований discovery, повноцінний соц-граф, зрілий admin back-office і три клієнти. **Проте продукт поки НЕ комерційний** через три критичні розриви:

- **(а) реальних грошей немає** — payout і refund це лише статуси/рядки в БД, без фактичного переказу провайдером;
- **(б) автентифікація** реалізована як email+password, тоді як `CLAUDE.md`/`spec.md` оголошують **єдиним дозволеним методом Google OAuth** (прямий конфлікт доктрини й коду);
- **(в) немає делегування ролей** — door-staff не може сканувати квитки без повного акаунта організатора.

Технічний борг **чесний і локалізований у грошовому seam** — дорога до запуску це не переписування, а замикання реального грошового транспорту, безпеки і compliance поверх зрілого каркаса.

---

## 2. Продуктова концепція та візія (розгорнуто)

### 2.1. Що це за продукт

Underground — **двосторонній маркетплейс подій із вбудованою соцмережею та ескроу-розрахунками**. Три сторони ринку:

| Сторона | Що отримує | Як монетизується платформа |
|---|---|---|
| **Попит** (відвідувачі) | Discovery, безпечна купівля (escrow), gift/refund, соц-граф | Прозора all-in комісія в UAH |
| **Пропозиція** (організатори) | Self-serve створення подій, кабінет, аналітика, виплати після події | 0% на безкоштовних подіях; апсели (early-payout, Pro-підписка) |
| **Дистрибуція** (промоутери — план) | Tracked-лінки, розщеплення комісії на escrow-seam | Через основну комісію з проданих квитків |

### 2.2. Три кити концепції

**Кит 1 — Discovery.** Персоналізований ранкінг подій: followed / near (Haversine-геодистанція) / interest / city / trending + momentum (продажі/збереження/відгуки) + recency. Анонім бачить trending. Це сильніше за плоский каталог UA-інкумбентів.

**Кит 2 — Квитки + Escrow.** Квиток як цифровий об'єкт з реальним QR; купівля проходить через ескроу. Гроші проходять стани `HELD → CLEARING → CLEARED → PAID_OUT`, з гілками `DISPUTED` (спір) і `REFUNDED` (повернення). Уся грошова логіка — через єдиний seam `api/src/escrow.rs`, що дозволяє замінити провайдера, не переписуючи бізнес-логіку.

**Кит 3 — Соцмережа.** Не «реєстрація на подію», а соц-об'єкт навколо квитка: профілі, підписки (follow), стрічка (пости підписок ∪ події за інтересами), лайки, коментарі, відгуки (1..5), DM-чат. Це канал росту, що знижує CAC проти платного трафіку.

### 2.3. Позиціонування

**«Соціальна афіша Києва, де гроші захищені до події».** Три вектори диференціації проти суто транзакційних UA-кас (Karabas, Concert.ua, Internet-bilet, Kontramarka, Ticketsbox):

1. **Довіра через escrow** — whitespace, який не комунікує жоден UA-гравець. Скасування = автоматичний повний рефанд, включно з сервісним збором.
2. **Соціальність як канал росту** — follow-організатора з push, «хто з друзів іде», стрічка активності, відгуки.
3. **Чесна економіка** — all-in прозора ціна в UAH (одна сума, без стеку «service + processing»), ефективна комісія нижча за Fever (15-30%), face-value resale як анти-скальпінг.

**Структурні переваги:** Android-first проти iOS-first Posh; вузький Київ-фокус для density/ліквідності; бренд «underground» природно лягає на кураторський шар про київську сцену.

**Ключова загроза позиціонуванню:** escrow не має перетворитись на «заморозку коштів організатора» (причина #1 відтоку від Eventbrite) — потрібен передбачуваний швидкий payout після `CLEARED` + платний early-payout як клапан cash-flow.

---

## 3. Цільові аудиторії та сценарії

### 3.1. Відвідувач (споживач)

| Сценарій | Стан реалізації |
|---|---|
| Знайти подію (афіша «Для тебе» + «Усі») | ✅ Реалізовано (`discover.rs`, `events.rs`) |
| Купити квиток | 🟡 Лише test-mode оплата; live checkout не замкнутий |
| Отримати безкоштовний квиток | ✅ Реалізовано (`tickets.rs` reserve) |
| Подарувати квиток (gift/transfer за email) | ✅ Реалізовано (`tickets.rs` transfer) |
| Повернути квиток (refund) | 🟡 Статус міняється в БД, реальних грошей немає |
| Відкрити спір (dispute) | ✅ Логіка є (`orders.rs` → frozen), реального рефанду немає |
| Соц-граф: підписки, стрічка, чат, відгуки | ✅ Реалізовано (`social.rs`, `posts.rs`, `messages.rs`, `reviews.rs`) |

### 3.2. Організатор

| Сценарій | Стан реалізації |
|---|---|
| Створити подію будь-якого масштабу | ✅ Будь-хто може (`events.rs:206`, `organizer_id=caller`) |
| Багаті поля події (медіа/гео/age) | ✅ Реалізовано |
| Кабінет: KPI, аналітика, відвідувачі | ✅ Web (`organizer.rs`); 🟡 Android (немає продажної аналітики) |
| Бачити статус виплат (HELD→...→PAID_OUT) | 🟡 Web read-only; на Android відсутнє |
| Ticket tiers (VIP/early-bird/звичайний) | ❌ Лише один price + tickets_total |
| KYC онбординг для виплат | 🟡 Loose-IBAN + ручний admin-verify наосліп |

### 3.3. Персонал / делегат (door-staff)

| Сценарій | Стан реалізації |
|---|---|
| Делегувати персоналу право сканувати квитки | ❌ **Не реалізовано** — лише ролі `USER`/`ADMIN` |
| Сканувати QR на вході | 🟡 Працює, але **лише для самого організатора** (`tickets.rs:179`) |

Це **прямий блокер операційного сценарію засновника**: реальний організатор мусить ділитися повним акаунтом з door-staff, що небезпечно.

### 3.4. Платформа-оператор (admin)

| Сценарій | Стан реалізації |
|---|---|
| Дашборд GMV / комісія / лічильники | ✅ (`admin.rs:26`) |
| Модерація користувачів / подій / контенту | ✅ |
| Черга скарг з enforcement | ✅ (`reports.rs`) |
| Ручний escrow refund / freeze / release | 🟡 Лише статуси в БД, без реального переказу |
| Payout ledger з KYC-gate | 🟡 Бухгалтерія без реального транспорту |
| Audit-trail | ✅ Append-only (`audit.rs`) |
| Reconciliation / anti-fraud | ❌ Відсутні |

---

## 4. Архітектура та поверхні

| Поверхня | Технології | Реальний стан | Зрілість |
|---|---|---|---|
| **API** | Rust + Axum + Tokio + SQLx/PostgreSQL (`api/`) | Найповніший шар: escrow-движок, webhook, соц-граф, admin-ендпоінти, фонові worker-и | ~62% |
| **Android** | Kotlin + Jetpack Compose, dark-only, 5 табів (`android/`) | Широкий Compose-клієнт; блокери — оплата test-only, Google-кнопка заглушка, немає door-scan делегування | 72% |
| **Web** | Next.js 15 / React 18, App Router (`web/`) | Афіша, SSR-подія з OG, мої квитки, кабінет організатора, SEO; блокер — checkout test-only | 72% |
| **Admin** | Next.js / React поверх `/api/admin/*` (`admin/`) | Найзріліша операційна площина; drawers для грошових дій | 62% |
| **Auth** | email+password (HS256 JWT) | **Конфлікт з доктриною**: spec вимагає Google OAuth only | 55% |
| **Платежі** | Lemon Squeezy (MoR), webhook-патерн | HMAC-verify, ідемпотентний fulfill; **реального транспорту грошей немає** | 60-62% |
| **Push** | FCM outbox-конвеєр | Конвеєр готовий, **FCM HTTP-транспорт — TODO-заглушка** (`main.rs:139`) | 55% |

**Фонові worker-и (`main.rs`):** escrow-sweep (`escrow.rs:10` — held→clearing→cleared), reminders-sweep, push-outbox (доставлення-заглушка).

---

## 5. Що вже зроблено — детально по областях

> Чесна примітка: «done» нижче означає «логіка реалізована й протестована на рівні БД/станів», **не** «працює з реальними грошима в проді». Грошові дії (`payout`, `refund`) — це наразі бухгалтерські рядки/статуси.

### 5.1. Escrow стейт-машина та грошова логіка — **62%**

Повний lifecycle `HELD → CLEARING → CLEARED → PAID_OUT` з фоновим `escrow_worker` sweep, гілки `DISPUTED` / `REFUNDED`, `frozen` виключено зі sweep, `platform_fee` на виплаті, ідемпотентний `fulfill` під `FOR UPDATE`.

| Доказ | Що підтверджує |
|---|---|
| `api/src/escrow.rs:10-34` | `sweep`: held→clearing коли подія завершилась, clearing→cleared після `clearing_window_days` |
| `api/src/main.rs:98` | фоновий worker викликає sweep інтервально |
| `api/src/payouts.rs:31-75` | `create_payout`: KYC+blocked гард, net = sum*(10000-bps)/10000 |
| `api/tests/escrow_m6.rs:123,148` | тести: sweep просуває стани; frozen ніколи не свіпається |

**КРИТИЧНО (stub):** реального переказу грошей немає — `payout` лише пише рядок і ставить orders `paid_out`; міграція `0018` робить `payouts.status DEFAULT 'paid'`, тобто виплата позначена оплаченою **до** фактичного переказу. Гроші переказує людина поза системою.

### 5.2. Lemon Squeezy webhook та checkout — **60%**

| Доказ | Що підтверджує |
|---|---|
| `api/src/webhooks.rs:28-35` | HMAC-SHA256 constant-time verify |
| `api/src/webhooks.rs:66` | SERIALIZABLE-tx ідемпотентність |
| `api/src/orders.rs:128-145` | live віддає hosted-checkout URL з `order_id` у custom_data |
| `api/tests/webhook.rs:126,196` | тести signature + idempotency |

**Прогалини:** обробляються лише `order_created` / `order_paid`; **refund/chargeback/dispute webhooks ігноруються (no-op)** (`webhooks.rs:54`); немає persistent webhook-event ledger і replay-захисту; MoR-сумісність з event-tickets+маркетплейс-виплатами не підтверджена.

### 5.3. Event lifecycle, discovery, пошук — **58%**

| Доказ | Що підтверджує |
|---|---|
| `api/src/events.rs:206-238` | будь-хто створює подію, organizer_id=caller |
| `api/migrations/0013_user_is_organizer.sql` | «anyone can create events», is_organizer косметичний |
| `api/src/discover.rs:39-87` | Haversine + blended score + social-proof + reason |
| `api/src/events.rs:139-183` | пошук city/category/free/q/sort, LIKE екранований |

**Прогалини:** немає ticket tiers (єдиний price+tickets_total); немає recurring/multi-session/seating/waitlist; пошук `q` лише по title; без пагінації (LIMIT 50); категорія — вільний текст без довідника.

### 5.4. Квитки: e-ticket/QR, gift, refund, dispute — **80%**

| Доказ | Що підтверджує |
|---|---|
| `android/.../util/Qr.kt` (zxing) | реальний QR-бітмап |
| `api/src/tickets.rs:90-147` | reserve free-квитка з dedup (unique index `0021`) |
| `api/src/tickets.rs:255-309` | transfer (gift за email) з age-gate + frozen-block |
| `api/src/tickets.rs:204-247` | self-return з escrow-рефандом (лише held\|clearing) |
| `api/src/tickets.rs:179-180` | scan/validate **лише для організатора події** |

**Прогалина (stub):** рефанд не повертає реальні гроші. **Блокер:** немає делегованого door-scan.

### 5.5. Соцмережа — **78%**

| Доказ | Що підтверджує |
|---|---|
| `api/src/social.rs:75-163` | ідемпотентні follow/unfollow з нотифікацією |
| `api/src/posts.rs:152-170` | feed = пости підписок ∪ події за інтересами з `reason` |
| `api/src/posts.rs` | пости/лайки/коментарі, ідемпотентні |
| `api/src/reviews.rs` | відгуки 1..5, upsert один на (event,author) |
| `api/src/messages.rs` | DM-чат (polling) |

**Прогалини:** чат без realtime (polling 4с), без блокування/мьюту; відгуки без перевірки відвідування (накрутка рейтингу); без пагінації; без медіа в постах.

### 5.6. Auth, акаунти, сесії — **55%**

| Доказ | Що підтверджує |
|---|---|
| `api/src/auth.rs:22-34` | argon2id хеш паролю |
| `api/src/auth.rs:46-71` | HS256 JWT з `tokens_valid_after` watermark |
| `api/src/passwords.rs` | password reset (анти-enumeration, 204 завжди) |
| `api/src/gdpr.rs` | GDPR erasure (Art.17) + export (Art.20) |

**КРИТИЧНО (конфлікт доктрини):** Google OAuth повністю відсутній попри те, що `CLAUDE.md`/`spec.md` оголошують його **єдиним** методом. Немає `/api/auth/google`, верифікації ID-token; `google_sub` завжди NULL (`routes.rs:110-115`). Реалізовано саме заборонену гілку. Android-кнопка Google = тост-заглушка (`AuthScreen.kt:113-118`). **Немає rate-limiting логіну (BR-017 порушено).**

### 5.7. Ролі, делегування, door-staff — **10%**

| Доказ | Що підтверджує |
|---|---|
| `api/src/auth.rs` (CurrentAdmin) | лише `USER` \| `ADMIN` |
| `api/src/tickets.rs:179-180` | scan дозволений лише `organizer_id == user.id` |

Немає ролі `ORGANIZER`, немає team/staff/delegation у міграціях. **Прямий блокер операційного сценарію засновника.**

### 5.8. Admin / back-office — **62%**

| Доказ | Що підтверджує |
|---|---|
| `api/src/admin.rs:26-39` | дашборд GMV/комісія/лічильники |
| `api/src/admin.rs:185-217` | модерація подій (published/hidden/blocked) |
| `api/src/admin.rs:270-356` | escrow refund/freeze/release (force) |
| `api/src/payouts.rs:125-161` | ручне керування payout (paid/failed/pending) |
| `api/src/audit.rs` | append-only audit-trail у тій самій tx |

**Прогалини:** refund/payout — лише статуси в БД; dispute-resolution = freeze-toggle (без черги/доказів/SLA); KYC-verify наосліп (admin не бачить bank-details — `kyc.rs:76-85`); немає reconciliation/export/anti-fraud; GMV змішує валюти (`admin.rs:32`); UI показує фільтр `cancelled`, який PATCH не приймає (`admin.rs:191`).

### 5.9. Android consumer app — **72%**

| Доказ | Що підтверджує |
|---|---|
| `AppScaffold.kt` | 5 табів, Navigation-Compose |
| `ui/detail/EventDetailScreen.kt:66-399` | детальна подія (постер/галерея/відео/карта/age/соц/share/reviews) |
| `ui/tickets/ETicketScreen.kt` | e-ticket + реальний QR; gift/return/dispute |
| `ui/events/ScanTickets.kt:42-96` | сканування (лише власник події) |

**Блокери:** оплата **лише test-mode** (`TestPayScreen` → `pay-test`, `CheckoutResp.mode` ніколи не перевіряється); Google-кнопка = тост; немає door-staff делегування; немає organizer-аналітики/payout-видимості на мобільному.

### 5.10. Web client + organizer cabinet — **72%**

| Доказ | Що підтверджує |
|---|---|
| `web/app/events/[eventId]/page.tsx` | SSR-сторінка події + generateMetadata (OG/Twitter) |
| `web/app/me/tickets/page.tsx` | мої квитки + QR + escrow-таймлайн |
| `web/app/organizer/**` | кабінет: KPI, створення/редагування, відвідувачі+CSV, промокоди, аналітика, payout read-only |
| `web/app/sitemap.ts`, `robots.ts` | динамічний SEO |

**Блокери:** checkout лише test-mode (`CheckoutResp` без `checkout_url`, `confirmBuy` завжди викликає `payTest`); промокод покупцем не застосовується; часткова i18n (сторінка відвідувачів + legal — UA-only); JWT у localStorage; **нуль web-тестів**.

### 5.11. FCM push та нотифікації — **55%**

| Доказ | Що підтверджує |
|---|---|
| `api/src/notifications.rs` | in-app нотифікації повні (uk/en, social + event-match + followers) |
| `api/src/push.rs`, `devices.rs` | outbox-конвеєр (selection/fan-out/watermark) готовий і протестований |
| `api/src/main.rs:139-143` | **FCM HTTP-транспорт — TODO-заглушка** (лише `tracing::info`) |

In-app нотифікації працюють; реального push-доставлення немає без Firebase-кредів.

### 5.12. KYC / організаторський onboarding — **35%**

| Доказ | Що підтверджує |
|---|---|
| `api/src/kyc.rs:19-25` | loose-IBAN валідація (без mod-97) |
| `api/src/kyc.rs:93-125` | admin pending→verified (наосліп) |
| `api/src/payouts.rs:38` | gate на payout |

**Прогалини:** немає документів, немає Sumsub/Onfido/Diia, немає AML. Нерозгортанно як MoR-маркетплейс з ескроу.

### 5.13. Процес, docs, CI/CD, legal — **50%**

| Доказ | Що підтверджує |
|---|---|
| `docs/spec.md` | 11 UC / 17 BR / 2 WF — зрілий бізнес-рівень |
| `docs/rules.md` (v0.5.0) | STRICT-доктрина, вичерпна |
| `docs/audit/*` | 7 grounded аудит-документів |

**Порушення власної доктрини (§11):** SDD для M1-M8 відсутні; роадмап живе лише в `docs/audit/`, не як `TASK-###` у backlog; немає `docs/ui-schematics/`, `docs/ui-baselines/`, `docs/ui-design-tokens.md`; **немає `.github/workflows`** (CI відсутній — 100% coverage недоказово); legal — TEMPLATE без counsel-review; M9 Figma-звірка BLOCKED.

---

## 6. Карта функцій vs задумана візія

| # | Функція (візія засновника) | Статус | Коментар (докази) |
|---|---|:---:|---|
| 1 | Будь-хто створює подію будь-якого масштабу | 🟡 | Створення ✅ (`events.rs:206`), але «будь-якого масштабу» блокується відсутністю ticket tiers/seating/recurring |
| 2 | Будь-хто купує квиток | 🟡 | Логіка ✅, але оплата лише test-mode на обох клієнтах — **реальних грошей платформа прийняти не може** |
| 3 | Ескроу: гроші утримуються до події | 🟡 | Стейт-машина ✅ (`escrow.rs`), але payout/refund — лише статуси в БД, без реального переказу |
| 4 | Виплата організатору після події | 🟡 | Бухгалтерія ✅ (`payouts.rs`), реальний транспорт ❌; `payouts.status DEFAULT 'paid'` приховує невиконані перекази |
| 5 | Нормальна адмінка/кабінет організатора | 🟡 | Web-кабінет ✅; payout-видимість read-only; Android без аналітики/виплат |
| 6 | Делегування персоналу (door-staff scan) | ❌ | Лише `USER`/`ADMIN`; scan лише організатору (`tickets.rs:179`) |
| 7 | Зручне знаходження подій (discovery) | ✅ | Сильний персоналізований ранкінг (`discover.rs`) |
| 8 | Купівля квитків | 🟡 | Див. #2 |
| 9 | Дарування квитків (gift) | ✅ | `tickets.rs:255` transfer за email з age-gate |
| 10 | Повернення квитків (refund) | 🟡 | Статус міняється (`tickets.rs:204`), реальних грошей не повертає |
| 11 | Відмінний мобільний UX | 🟡 | Широкий Compose-клієнт ✅, але платіж/Google/door-scan — діри |
| 12 | Типи квитків / «пакети» | ❌ | Лише один price + tickets_total; промокоди дають лише %-знижку |
| 13 | E-ticket + QR + валідація на вході | ✅ | Реальний QR (zxing) + scan/validate працюють |
| 14 | Відгуки (reviews) | 🟡 | 1..5 ✅, але без перевірки відвідування — ризик накрутки |
| 15 | Стрічка (feed) активності підписок | ✅ | `posts.rs:152` — пости підписок ∪ події за інтересами |
| 16 | Коментарі до постів у стрічці | ✅ | `posts.rs` лайки/коментарі ідемпотентні |
| 17 | Google OAuth як метод входу | ❌ | Відсутній; реалізовано email+password (заборонена доктриною гілка) |
| 18 | Комерційно успішний продукт (live money) | ❌ | Блокується #2/#3/#4 + відсутністю KYC/AML, anti-fraud, CI, legal |

**Легенда:** ✅ реалізовано / 🟡 частково або лише на рівні БД-логіки / ❌ відсутнє.

---

## 7. Сильні сторони, технічний борг та ризики

### 7.1. Сильні сторони

1. **Escrow-движок — справжня стейт-машина, не слайди.** Фоновий sweep, ідемпотентність під `FOR UPDATE`, гілки DISPUTED/REFUNDED, frozen-виключення, platform-fee на виплаті. Це найскладніша частина — і вона є.
2. **Єдиний грошовий seam (`escrow.rs`).** Дозволяє замкнути реальний транспорт (monobank/IBAN/Stripe Connect) без переписування бізнес-логіки.
3. **Зрілий webhook-патерн.** HMAC-SHA256 constant-time verify + SERIALIZABLE-ідемпотентність — production-grade основа.
4. **Повноцінний соц-граф.** Follows, feed з reason, лайки/коментарі/відгуки, DM, нотифікації uk/en — рідкісно повний для pre-launch.
5. **Сильний персоналізований discovery.** Haversine-гео + social-proof + momentum — диференціація проти плоских UA-кас.
6. **Три робочі клієнти + зрілий admin.** Android, Web, Admin — не прототипи, а широкі застосунки з реальними ендпоінтами.
7. **GDPR з дня 1.** Erasure (Art.17) + export (Art.20), фінансові записи retained і де-лінковані від PII.
8. **Append-only audit-trail** у тій самій транзакції, що й мутація.

### 7.2. Технічний борг та ризики (за критичністю)

| Серйозність | Ризик | Доказ | Наслідок |
|:---:|---|---|---|
| **P0** | Немає реального payout-транспорту | `payouts.rs:31`; міграція `0018` `DEFAULT 'paid'` | Розбіжність бухгалтерії й реальних грошей; подвійні/пропущені виплати |
| **P0** | Refund не повертає реальні гроші | `tickets.rs:239`, `admin.rs:315` | Покупець бачить «refunded» без коштів — юр/репутаційний/chargeback ризик |
| **P0** | Live-платежі не замкнуті на обох клієнтах | `CheckoutResp` без `checkout_url`; завжди `pay-test` | Платформа фізично не приймає гроші в проді |
| **P0** | MoR-несумісність (структурний) | Paddle забороняє event-tickets+marketplace; LS digital-only | Може змусити змінити весь грошовий рейк |
| **P0** | Google OAuth: доктрина vs код | `routes.rs:110`; `AuthScreen.kt:113` | Docs↔код розходяться; прихована бомба при додаванні OAuth |
| **P0** | Немає делегування door-staff | `tickets.rs:179` | Організатор мусить ділитися повним акаунтом |
| **P0** | Chargeback/dispute webhooks ігноруються | `webhooks.rs:54` | При chargeback гроші можуть бути вже paid_out — втрата коштів |
| **P0** | Немає реального KYC/AML | `kyc.rs:19,93` | Compliance-блокер для MoR-маркетплейсу з ескроу |
| **P0** | Немає CI/CD | відсутній `.github/workflows` | 100% coverage недоказово; репо не «Done» за §11 |
| **P0** | Немає anti-fraud/rate-limiting | grep governor = 0 | Brute-force логіну (BR-017), карт-фрод, боти на гарячих продажах |
| **P0** | Немає ops-фундаменту | немає `/readiness`, graceful shutdown, backups | Інцидент БД може пошкодити escrow-стан |
| **P0** | Legal — TEMPLATE | `docs/legal/*` | Escrow в UA = зона НБУ; live без юрвисновку — регуляторний ризик |
| **P1** | Dispute-resolution = freeze-toggle | `admin.rs:323` | Немає черги/доказів/SLA/нотифікацій сторін |
| **P1** | Немає ticket tiers | `0001_init.sql` | Критично для платних івентів реального масштабу |
| **P1** | Organizer летить наосліп | payout read-only, немає мобільної аналітики | Блокер довіри маркетплейсу |
| **P1** | Reviews без перевірки відвідування | `reviews.rs` | Накрутка рейтингу |
| **P1** | Чат без realtime/блокування | `messages.rs` polling | Спам/харасмент-ризик |
| **P2** | Без пагінації (LIMIT 50) | `discover.rs`, `events.rs` | Деградація на масштабі |
| **P2** | JWT у localStorage, CSV-injection, UGC без санітизації | web-клієнт | Вектори перед публічним запуском |
| **P2** | Multi-currency неузгоджено | `admin.rs:32`, хардкод `₴` | Некоректна звітність |

### 7.3. Підсумкова оцінка зрілості

| Область | Зрілість |
|---|:---:|
| Квитки (e-ticket/QR/gift/dispute) | 80% |
| Соцмережа | 78% |
| Android / Web клієнти | 72% / 72% |
| Escrow / Admin | 62% / 62% |
| Lemon Squeezy webhook | 60% |
| Auth / Push | 55% / 55% |
| Процес/docs/legal | 50% |
| KYC/onboarding | 35% |
| Ролі / делегування | **10%** |

**Висновок.** Underground — це зрілий, чесно побудований каркас із реальною escrow-логікою, повним соц-графом і трьома робочими клієнтами. Технічний борг **сконцентрований у грошовому seam, безпеці й compliance** — і це добра новина: дорога до комерційного запуску це **замикання реального грошового транспорту, автентифікації, делегування ролей та compliance поверх уже-working ядра**, а не переписування продукту. Найбільші пріоритети (P0) — реальні payout/refund/live-checkout, валідація MoR-сумісності, вирішення auth-доктрини, делегування door-staff, KYC/AML, anti-fraud, CI та legal.