# Underground — стратегічний бекбон (синтез аналізу)

> Згенеровано багатоагентним аналізом 2026-06-29 (7 рідерів коду + 5 ринкових ресерчерів).

## Концепція

Underground — це Kyiv-first, mobile-native (Android-first) платформа, що зрощує три речі, які на UA-ринку досі живуть окремо: (1) персоналізоване discovery подій, (2) квитковий сервіс із e-ticket/QR, і (3) соціальний шар (профілі, підписки, стрічка, відгуки, чат). Стрижнева відмінність — escrow-маркетплейс: покупець платить, гроші УТРИМУЮТЬСЯ (HELD→CLEARING→CLEARED→PAID_OUT, гілки DISPUTED/REFUNDED через єдиний seam api/src/escrow.rs), і виплачуються організатору лише ПІСЛЯ події. Це перетворює головний біль UA-ринку (неповоротний 10% збір Karabas/Concert.ua і повільні повернення «за листом організатора») на trust-фічу: «платиш безпечно — організатор отримує гроші лише після того, як подія відбулась; скасування = автоматичне повне повернення».

Реальність кодової бази підтверджує, що це НЕ слайди, а працюючий каркас: повний escrow-стейт-машина з фоновим worker-sweep, ідемпотентний Lemon Squeezy webhook з HMAC-перевіркою, refund/dispute/transfer (gift) флоу, персоналізований discovery з Haversine-геозваженням і social-proof, повноцінний соц-граф (follows, feed = пости підписок ∪ події за інтересами, лайки, коментарі, відгуки 1..5, DM-чат), зрілий admin-back-office і три клієнти (Android, web, admin). Це рідкісно повний для pre-launch продукт.

Проте задумана візія засновника («будь-хто створює подію будь-якого масштабу, будь-хто купує/дарує/повертає, нормальна адмінка організатора, делегування door-staff, відмінний мобільний UX, базова соцмережа, комерційний успіх») розходиться з реальністю у трьох критичних місцях, які роблять продукт ПОКИ що не-комерційним: (а) реальних грошей немає — payout і refund це лише рядки/статуси в БД, без фактичного переказу провайдером; (б) автентифікація реалізована email+password, тоді як CLAUDE.md/spec оголошують ЄДИНИМ дозволеним методом Google OAuth (прямий конфлікт доктрини й коду); (в) немає делегування/ролей організатора — door-staff не може сканувати квитки без повного акаунта організатора. Плюс цілий клас «непарадних» обов'язкових речей (live-платежі end-to-end, chargeback-обробка, реальний KYC/AML, anti-fraud/anti-bot, rate-limiting, CI/CD, ops-фундамент, фіналізація legal) ще відсутній.

Уточнена концепція: Underground позиціонується як соц-first антитеза транзакційним UA-касам — escrow-захист покупця + face-value resale (анти-скальпінг у дусі DICE) + промоутерська/реферальна мережа на escrow-seam (у дусі Posh) + календарі-підписки на організатора (Luma) + соц-об'єкти навколо квитка «хто ще йде» (Partiful). Свідомо вузький старт: Київ, independent/клубна/концертна сцена, адаптований воєнний формат як частина ціннісної пропозиції. Технічний борг чесний і локалізований у грошовому seam — це означає, що дорога до комерційного запуску — це не переписування, а замикання реального грошового транспорту, безпеки і compliance поверх уже зрілого каркаса.

## Позиціонування

Позиціонування: «Соціальна афіша Києва, де гроші захищені до події». Три вектори диференціації проти UA-інкумбентів (Karabas, Concert.ua, Internet-bilet, Kontramarka, Ticketsbox), які всі є суто транзакційними касами без соц-шару, без escrow-захисту й без легального вторинного ринку:

1) ДОВІРА через escrow (whitespace, який не комунікує ЖОДЕН UA- і майже жоден глобальний гравець): гроші HELD до завершення події; скасування = автоматичний повний рефанд, включно з сервісним збором — прямий удар по неповоротному 10% збору й поверненням «за листом організатора» у Karabas/Concert.ua. Кодифікувати воєнне правило (тривога >2 год після проходження 70-80% події = подія відбулась) у escrow як автоматичне вирішення спорів через air-alert фід — фіча, якої немає ні в кого.

2) СОЦІАЛЬНІСТЬ як канал росту (whitespace на UA-ринку): follow-організатора з авто-push про нові події (Luma Community Calendars), «хто з твоїх друзів іде» на сторінці події (Partiful — доведений #1 conversion-двигун), стрічка активності підписок, відгуки. Це знижує CAC проти платного трафіку, якого потребують legacy-каси.

3) ЧЕСНА ЕКОНОМІКА: all-in прозора ціна в UAH (одна сума, без стеку «service + processing», як DICE/Posh/Luma — дає ~30% вищу конверсію), ефективна комісія помітно нижча за Fever 15-30%, ближче до нижнього діапазону Posh з поправкою на UA-купівельну спроможність; face-value resale (стеля = номінал) як анти-скальпінг і захист бренду «underground».

Структурні переваги для виконання: Android-first проти iOS-first Posh (на UA-ринку домінує Android — пряма дистрибуційна перевага); вузький Київ-фокус дає density/ліквідність; бренд «underground» природно лягає на кураторський шар про київську сцену (RA-стиль) для органічного SEO. Ключова загроза позиціонуванню: escrow не має перетворитись на «заморозку коштів організатора» (причина #1 відтоку від Eventbrite) — треба гарантувати передбачуваний швидкий payout після CLEARED + платний early-payout як клапан для cash-flow організаторів.

## Що зроблено (зрілість)

- **Escrow стейт-машина та грошова логіка (БД-рівень)** — 62%: Повний lifecycle HELD→CLEARING→CLEARED→PAID_OUT з фоновим escrow_worker sweep, гілки DISPUTED/REFUNDED, frozen виключено зі sweep, platform_fee на виплаті, ідемпотентний fulfill під FOR UPDATE. КРИТИЧНО: реального переказу грошей немає — payout і refund лише пишуть рядок/статус у БД; payouts.status DEFAULT 'paid' приховує невиконані перекази.
- **Lemon Squeezy webhook та checkout** — 60%: HMAC-SHA256 constant-time verify, SERIALIZABLE-tx ідемпотентність, test-mode pay-test чітко відділено від live, live віддає hosted-checkout URL. Але: обробляються лише order_created/order_paid; refund/chargeback/dispute webhooks ігноруються (no-op); немає persistent webhook-event ledger і replay-захисту; MoR-сумісність з event-tickets+маркетплейс-виплатами не підтверджена.
- **Event lifecycle, discovery, пошук** — 58%: Будь-хто створює подію (organizer_id=caller), багаті поля (медіа/geo/age_rating), owner-only update, soft-cancel. Сильний персоналізований discovery (Haversine+social-proof+momentum). Але: немає ticket tiers (єдиний price+tickets_total), recurring/multi-session/seating/waitlist; пошук лише по title; без пагінації (LIMIT 50).
- **Квитки: e-ticket/QR, gift, refund, dispute** — 80%: Реальний QR (zxing), reserve free-квитка з dedup, transfer (gift за email) з age-gate+frozen-block, self-return з escrow-рефандом (held|clearing), buyer-dispute→frozen. Валідація на вході (scan) працює, але лише для власника події. Рефанд не повертає реальні гроші.
- **Соцмережа (профілі, follows, feed, пости, коментарі, відгуки, DM)** — 78%: Повноцінний соц-граф: ідемпотентні follow/unfollow з нотифікацією, feed = пости підписок ∪ події за інтересами з reason, лайки/коментарі, відгуки 1..5 (upsert), DM-чат (polling). Прогалини: чат без realtime/блокування, відгуки без перевірки відвідування, без пагінації, без медіа в постах.
- **Auth, акаунти, сесії** — 55%: Email+password (argon2id), HS256 JWT з tokens_valid_after watermark, password reset (анти-enumeration), GDPR erasure+export. КРИТИЧНО: Google OAuth повністю відсутній попри те, що CLAUDE.md/spec оголошують його ЄДИНИМ методом — реалізовано саме заборонену гілку. Немає rate-limiting логіну (BR-017 порушено).
- **Ролі, делегування, door-staff** — 10%: Лише USER|ADMIN. Немає ролі ORGANIZER, немає team/staff/delegation. Сканувати квиток може ЛИШЕ організатор події (tickets.rs:179) — door-staff неможливий без шарингу повного акаунта. Прямий блокер операційного сценарію засновника.
- **Admin / back-office** — 62%: Найзріліша площина: дашборд GMV/комісія, модерація користувачів/подій/контенту, черга скарг з enforcement, ручний escrow refund/freeze/release, payout ledger з KYC-gate, audit-trail. Прогалини: немає реальних refund/payout-переказів, dispute-resolution лише freeze-toggle, KYC наосліп, немає reconciliation/export/anti-fraud.
- **Android consumer app** — 72%: Широкий Compose-клієнт (dark-only, 5 табів): онбординг, афіша+discovery, детальна подія, e-ticket+QR, gift/return/dispute, feed/пости/коментарі, профілі, чат, нотифікації, налаштування+KYC+GDPR, створення/керування подіями, scan. Блокери: оплата ЛИШЕ test-mode (немає LS WebView), Google-кнопка=тост-заглушка, немає door-staff делегування, немає organizer-аналітики на мобільному.
- **Web client + organizer cabinet** — 72%: Next.js 15: афіша/discovery, SSR-сторінка події з OG, мої квитки+QR+escrow-таймлайн, кабінет організатора (KPI, створення/редагування, відвідувачі+CSV, промокоди, аналітика, payout-history read-only), SEO (sitemap/robots). Блокери: checkout лише test-mode (CheckoutResp без checkout_url), промокод покупцем не застосовується, часткова i18n, JWT у localStorage, нуль web-тестів.
- **FCM push та нотифікації** — 55%: In-app нотифікації повні (uk/en, social+event-match+followers, reminders sweep). Push outbox-конвеєр (selection/fan-out/watermark) готовий і протестований, але FCM HTTP-транспорт — TODO-заглушка (main.rs:139): реального доставлення немає без Firebase-кредів.
- **KYC / організаторський onboarding** — 35%: Збір IBAN/SWIFT + admin pending→verified, gate на payout. Але: IBAN loose-валідація (без mod-97), верифікація вручну наосліп (адмін не бачить bank-details), немає документів, немає Sumsub/Onfido, немає AML — нерозгортанно як MoR-маркетплейс з ескроу.
- **Процес, docs, CI/CD, legal** — 50%: spec.md (11 UC/17 BR/2 WF) і rules.md (v0.5.0 STRICT) зрілі. Але порушення власної доктрини: SDD для M1-M8 відсутні, роадмап живе лише в audit/, немає ui-schematics/baselines/design-tokens, немає .github/workflows (CI відсутній — 100% coverage недоказово), legal — TEMPLATE без counsel-review, M9 Figma-звірка BLOCKED.

## Прогалини (за пріоритетом)

- **[P0] Реальний грошовий транспорт (payout)** — Payout не викликає жодного банк/LS API — лише пише рядок і ставить orders paid_out; payouts.status DEFAULT 'paid' (міграція 0018) позначає виплату оплаченою до фактичного переказу. Гроші організатору переказує людина поза системою.
  - → Інтегрувати реальний виплатний рейк (локальний UA: monobank/IBAN UAH, або Stripe Connect transfers). Виправити default на 'pending'; перехід у 'paid' прив'язати до підтвердженого переказу. Зберегти escrow.rs як єдину абстракцію над провайдером.
- **[P0] Реальний refund через провайдера** — refund/return лише змінює escrow_status='refunded' у БД (tickets.rs:239, admin.rs:315) — покупцю гроші НЕ повертаються. Покупець бачить 'refunded' без коштів. Юридичний/репутаційний/chargeback ризик.
  - → Підключити LS/еквайр refund API; статус 'refunded' має ставитись лише після підтвердження повернення провайдером.
- **[P0] Live-платежі end-to-end (обидва клієнти)** — Web confirmBuy завжди викликає pay-test; CheckoutResp не містить checkout_url. Android завжди веде в TestPayScreen. У live-режимі купівля зламається — платформа фізично не може приймати гроші в проді.
  - → Прокинути checkout_url у CheckoutResp; web редірект на LS hosted-checkout + success/return-сторінка; Android — WebView/Custom Tab на checkout_url з перевіркою CheckoutResp.mode. End-to-end smoke проти реального LS.
- **[P0] MoR-сумісність (структурний ризик)** — Lemon Squeezy/Paddle AUP орієнтовані на digital SaaS; Paddle прямо забороняє event tickets+маркетплейс зі сторонніми виплатами, LS — digital-only/no third-party marketplace. Модель Underground виходить за межі типового MoR — це архітектурний, не технічний блокер escrow.rs.
  - → НЕГАЙНО письмово валідувати з LS/Stripe дозвіл на (а) продаж event-tickets, (б) виплати третім особам. Підготувати План Б (UA-еквайр LiqPay/Fondy/WayForPay або Stripe Connect). Не хардкодити LS у грошову логіку.
- **[P0] Google OAuth (доктрина vs код)** — CLAUDE.md/spec оголошують Google OAuth ЄДИНИМ методом входу; реалізовано email+password (заборонена гілка). Немає /api/auth/google, верифікації ID-token; google_sub завжди NULL; Android-кнопка=тост.
  - → Засновник має вирішити доктрину (див. open_questions). Якщо Google — пріоритет: реалізувати POST /api/auth/google з верифікацією ID-token + лінкування за email; інакше — оновити spec/CLAUDE.md, додати rate-limit на email-login.
- **[P0] Делегування / door-staff ролі** — Сканувати квиток може лише організатор події; немає ролі ORGANIZER/STAFF/delegation. Реальний організатор мусить ділитися повним акаунтом — блокує операційний сценарій ticketing і небезпечно.
  - → Додати модель team/staff з делегованим scoped-правом (validate-only на конкретні події організатора), без доступу до фінансів/редагування. UI делегування + door-scan на мобільному.
- **[P0] Chargeback / dispute webhooks** — LS refund/chargeback/dispute-вебхуки ігноруються (webhooks.rs:54 — лише order_created/order_paid). При chargeback гроші можуть бути вже cleared/paid_out — платформа втрачає кошти без авто-freeze.
  - → Обробляти order_refunded/dispute/chargeback з авто-freeze ескроу; persistent webhook-event ledger з idempotency-key + replay/timestamp захистом.
- **[P0] Реальний KYC/AML** — KYC лише loose-IBAN + ручне admin-verify наосліп; немає документів, identity-провайдера, AML-моніторингу. Виплати реальним людям без справжнього KYC — compliance-блокер для MoR-маркетплейсу з ескроу.
  - → Інтегрувати Sumsub/Onfido (або Diia.Підпис/Diia.Card для UA); mod-97 IBAN; KYB для організаторів (ЄДРПОУ/ІПН/бенефіціари); KYB-gate на CLEARED→PAID_OUT.
- **[P0] CI/CD та доведена якість** — Немає .github/workflows — заявлені 100% coverage (rules §6) недоказові в CI; за власною доктриною §11 репо не 'Done'.
  - → Підняти CI pipeline (lint+test+coverage gate+security audit) як симуляцію scripts/ci.sh; fail-fast стадії.
- **[P0] Anti-fraud / anti-bot / rate-limiting** — Немає velocity/risk-scoring/blacklist; немає rate-limiting (логін, checkout); немає anti-bot/черги на гарячі продажі. >80% трафіку гарячих подій — боти; платіжна платформа з ескроу без risk-контролю вразлива до карткового фроду й відмивання.
  - → Rate-limiting (governor) на auth/checkout; device-fingerprinting; ліміт квитків на акаунт; черга на сплески; velocity-чеки на замовлення/виплати; 3DS/SCA.
- **[P0] Ops-фундамент (prod)** — Немає graceful shutdown, backups, /readiness, observability/metrics. Інцидент БД може пошкодити escrow-стан; рутинний деплой ризикований для грошей.
  - → Додати /readiness+/health, graceful shutdown, автобекапи, метрики (latency/errors/escrow-sweep стан), алерти на застряглі кошти.
- **[P0] Legal фіналізація** — Усі політики (privacy/terms/refund/abuse) позначені TEMPLATE — без counsel-review. Escrow/held-funds у UA — зона регулювання НБУ (тримання чужих коштів). GDPR-аналог УЗ №8153 (штраф до 5% обороту).
  - → UA-юрвисновок щодо статусу платіжних послуг НБУ й escrow-арбітражу ПЕРЕД live-ескроу; counsel-review усіх політик; чітка межа відповідальності LS↔Underground.
- **[P1] Dispute-resolution workflow** — Спір = лише escrow_status='frozen' + ручний admin escrow-toggle. Немає черги спорів, причин/доказів від сторін, дедлайнів, окремих результатів (на користь покупця/організатора), нотифікацій сторін.
  - → Побудувати dispute-модель: причина/докази, SLA-дедлайни, статуси рішення, двостороння комунікація, авто-нотифікації; кодифікувати воєнне правило (air-alert) як сигнал авто-вирішення.
- **[P1] Ticket tiers / типи квитків** — Лише один price+tickets_total на подію; промокоди дають лише %-знижку. Критично для будь-якого платного івенту реального масштабу (VIP/early-bird/звичайний).
  - → Додати ticket tiers з per-tier capacity/ціною/вікном продажу; узгодити з escrow/refund-логікою.
- **[P1] Organizer payout-history + analytics UI** — Організатор не бачить стану виплат (HELD→...→PAID_OUT) і повної аналітики на мобільному; web payout-history read-only. Для payout-маркетплейсу організатор 'летить наосліп' — блокер довіри.
  - → Organizer-scoped payout-history endpoint+UI на обох клієнтах; продажна аналітика (продано/виручка/відвідувачі/escrow-розбивка) на мобільному.
- **[P1] Промокоди на checkout (web)** — Організатор створює промокоди, але покупець на web не може їх застосувати (?promo не передається, UI вводу відсутній) — втрачена заявлена фіча конверсії/монетизації.
  - → Додати поле вводу промокоду на сторінці події + прокидання у checkout на обох клієнтах.
- **[P1] Face-value resale (P2P вторинний ринок)** — Немає легального вторинного ринку — біла пляма всіх UA-конкурентів і природний союзник escrow/анти-скальпінгу. Зараз перепродаж іде в Telegram (де й шахрайство).
  - → Закритий P2P-resale за номіналом (стеля=ціна), квиток повертається в систему й перевипускається, комісія платформи 5-10% — дохід+анти-спекуляція+захист бренду.
- **[P1] Real-time чат + модерація DM** — Чат лише polling (4с), без блокування/мьюту; будь-хто може писати будь-кому — спам/харасмент-ризик для соц-маркетплейсу.
  - → WS/SSE для realtime; блокування/мьют співрозмовника; rate-limit DM.
- **[P1] Reviews без перевірки відвідування** — Будь-хто може лишити відгук (reviews.rs) — накрутка рейтингу організатора.
  - → Gate: відгук лише для власника квитка на подію, що відбулась.
- **[P1] Age-assurance для 18+/21+** — Age-gate базується на самозаявленому dob без верифікації; на вході ID не перевіряється. Юридичний ризик для алко/нічних подій; DSA/UK OSA забороняють self-declared чекбокс.
  - → Реальний age-assurance для restricted-подій (age-estimation/ID або Diia); дублювати age-gate на сервері для reserve free-квитків.
- **[P1] Anti-scalping закритий контур** — Квиток (gift/transfer) передається досить вільно; немає прив'язки до особи в дусі DICE — вектор скальпінгу.
  - → Іменні квитки прив'язані до акаунта; transfer лише через систему; resale лише face-value через waitlist.
- **[P2] Пагінація / масштаб discovery** — discover/events без пагінації (LIMIT 50), Haversine+subqueries по всій таблиці на кожен запит; sweep повний UPDATE...JOIN без батчингу; fan-out нотифікацій без батчингу.
  - → Cursor-пагінація; матеріалізація/кеш ранкінгу; батчинг sweep і fan-out.
- **[P2] Full-text пошук + довідник категорій** — q лише по title, city/category точне співпадіння, категорія — вільний текст (розсинхрон фільтрів).
  - → Full-text (description+теги), керований довідник категорій, date-range/geo фільтри.
- **[P2] Медіа-pipeline (resize/CDN/scan)** — Локальний диск, content-type від клієнта (підробляється), без resize/CDN/контент-модерації; next images.unoptimized=true.
  - → CDN+image-resize+антивірус/модерація завантажень; серверна перевірка вмісту.
- **[P2] CSV-injection + JWT у localStorage + UGC-санітизація** — CSV-експорт відвідувачів не екранує коми/лапки (injection); JWT у localStorage (XSS-крадіжка); user-content рендериться без явної санітизації.
  - → Екранування CSV; httpOnly cookie для токена; санітизація/екранування UGC.
- **[P2] Multi-currency узгодженість** — Дашборд GMV змішує валюти (sum без розбивки), payout per-currency, ціна в /me/tickets захардкоджена в ₴ — мультивалютна звітність неузгоджена.
  - → Per-currency агрегація скрізь; fmtPrice замість хардкоду; FX-політика.
- **[P2] Accessibility WCAG 2.1 AA** — Немає підтвердженої доступності; dark-only тема має проходити contrast-ratio; TalkBack/focus order не перевірені. EAA-вимога при EU-амбіціях.
  - → WCAG 2.1 AA / EN 301 549 аудит Android+web; доступний checkout.
- **[P2] Процесні артефакти (rules §11)** — SDD для M1-M8 відсутні, роадмап не в backlog як TASK-###, немає ui-schematics/baselines/design-tokens, M9 Figma BLOCKED — за власною доктриною репо не 'Done'.
  - → Ретроспективно завести TASK+SDD для реалізованого; створити обов'язкові §11 артефакти; розблокувати M9.

## Дорожня карта

### M0 — Process & truth reconciliation — Привести docs у відповідність до коду й розблокувати дисципліну rules.md перед будь-яким новим грошовим кодом
_rules.md §0/§3/§12 роблять backlog+speck передумовою коду; MoR-валідація — архітектурний блокер, який може змінити весь грошовий seam. Без цього подальша робота порушує власну доктрину й ризикує переробкою._
- Вирішити доктрину Auth (Google-only vs email+Google) і синхронізувати CLAUDE.md/spec.md з реальністю
- Завести TASK-### + SDD-### для вже реалізованих M1-M8 (ретроспективно) і для всіх P0-задач нижче
- Створити обов'язкові §11 артефакти: docs/ui-schematics/, docs/ui-baselines/, docs/ui-design-tokens.md
- Підняти CI pipeline (.github/workflows): lint+test+coverage-gate+security-audit, fail-fast
- Письмово валідувати MoR-сумісність з LS/Stripe (event-tickets + third-party payouts)

### M1 — Real money seam (escrow→provider) — Замкнути фактичний рух грошей: live-checkout, реальні payout і refund, chargeback-обробка
_Без реальних грошей продукт не комерційний (P0). Це найбільший залишковий ризик ('real-money correctness can't be proven in CI'). Escrow вже існує — потрібно лише замкнути транспорт, тому ROI найвищий._
- Прокинути checkout_url у CheckoutResp; web-редірект на LS hosted-checkout + success/return; Android WebView/Custom Tab з перевіркою mode
- Інтегрувати реальний payout-рейк (UA monobank/IBAN UAH або Stripe Connect) за абстракцією escrow.rs; виправити payouts.status default→'pending'
- Реальний refund через провайдера; 'refunded' лише після підтвердження
- Обробка LS refund/chargeback/dispute webhooks з авто-freeze; persistent webhook-event ledger + idempotency-key + replay-захист
- End-to-end smoke проти реального LS (live) + reconciliation внутрішній стан↔провайдер
- SDD з SEC-векторами на кожну задачу; 100% unit+integration; [SEC] негативні вектори

### M2 — Trust, safety & compliance baseline — Зробити non-negotiable речі платіжної платформи: KYC/KYB, anti-fraud, age, legal
_Escrow + виплати третім особам = money-transmission/AML-зона; self-serve онбординг — головний вектор фроду. Ці речі — передумова легального live-запуску з реальними грошима (P0), не пост-фактум._
- Реальний KYC/KYB (Sumsub/Onfido або Diia.Підпис/Diia.Card); mod-97 IBAN; KYB-gate на CLEARED→PAID_OUT
- Rate-limiting (governor) на auth+checkout; 3DS/SCA; velocity/risk-scoring на замовлення й виплати
- Реальний age-assurance для 18+/21+; серверне дублювання age-gate на reserve
- Ops-фундамент: /readiness, graceful shutdown, backups, метрики, алерти на застряглі кошти
- UA-юрвисновок (НБУ/escrow-арбітраж) + counsel-review усіх legal-політик; межа відповідальності LS↔Underground

### M3 — Organizer operations completeness — Зробити організатора повноцінним: делегування door-staff, payout-видимість, ticket tiers, dispute-resolution
_Засновницька візія прямо вимагає door-staff делегування й нормальної адмінки організатора; ticket tiers критичні для платних івентів реального масштабу. Без payout-видимості організатор 'летить наосліп' — блокер довіри маркетплейсу._
- Модель team/staff з делегованим scoped validate-only правом; door-scan делегування на мобільному
- Organizer payout-history endpoint+UI (web+Android) + продажна аналітика на мобільному
- Ticket tiers (per-tier capacity/ціна/вікно продажу), узгоджені з escrow/refund
- Dispute-resolution workflow: причини/докази/SLA/результати/нотифікації; air-alert авто-вирішення
- Промокоди на checkout (web+Android)

### M4 — Social growth loops & discovery — Активувати соц-first зростання — головну диференціацію проти UA-кас
_Соц-граф уже побудований, але growth-петлі (guest-list, follow-push, promoter-network) не активовані — це найдешевший канал росту проти платного трафіку й whitespace на UA-ринку. FCM-транспорт — остання миля для реальних нотифікацій._
- 'Хто з друзів іде' на сторінці події (доведений #1 conversion-двигун); видимий guest-list
- Follow-організатора з реальним FCM push про нові події (замкнути FCM-транспорт у main.rs)
- Promoter/Kickback-мережа на escrow-seam: tracked-лінки, розщеплення комісії промоутеру
- Realtime чат (WS/SSE) + блокування/мьют DM; reviews лише для відвідувачів
- Full-text пошук + довідник категорій + cursor-пагінація discovery/feed

### M5 — Differentiation & scale polish — Закрити білі плями ринку й підготувати масштаб
_Face-value resale і refund-protection — найбезпечніші додаткові доходи + захист бренду; early-payout знімає головну причину відтоку організаторів від held-funds. Це монетизаційний і масштабний шар поверх уже-working ядра._
- Face-value P2P-resale (закритий контур, стеля=номінал, комісія 5-10%) + іменні квитки (анти-скальпінг DICE-стиль)
- Refund-protection revenue-share на checkout (Cover Genius/XCover або власний cancellation-fund)
- Early/instant payout за +3% (клапан cash-flow організаторів)
- Кураторський/редакційний шар про київську сцену (RA-стиль) для органічного SEO; JSON-LD/hreflang
- Медіа-pipeline (CDN/resize/scan); accessibility WCAG 2.1 AA; multi-currency узгодженість; повна i18n
- Розблокувати M9 Figma-звірку (44 фрейми)

## Додати

- POST /api/auth/google з верифікацією ID-token + лінкування за email (якщо доктрина Google-only)
- Реальний payout-транспорт (monobank/IBAN UAH або Stripe Connect) за абстракцією escrow.rs
- Реальний refund через провайдера (LS/еквайр API)
- checkout_url у CheckoutResp + live hosted-checkout флоу (web redirect, Android WebView)
- Обробка LS refund/chargeback/dispute webhooks з авто-freeze + persistent webhook-event ledger
- Модель ролей team/staff з делегованим scoped door-scan правом
- Реальний KYC/KYB (Sumsub/Onfido/Diia) + mod-97 IBAN + KYB-gate на payout
- Rate-limiting (governor) на auth+checkout; 3DS/SCA; velocity/risk-scoring
- Ticket tiers (VIP/early-bird/звичайний) з per-tier capacity/ціною/вікном
- Dispute-resolution workflow (причини/докази/SLA/результати/нотифікації) + air-alert авто-вирішення
- Organizer payout-history endpoint+UI та продажна аналітика на мобільному
- 'Хто з друзів іде' / видимий guest-list на сторінці події
- Follow-організатора push (замкнути FCM HTTP-транспорт у main.rs)
- Promoter/Kickback-мережа на escrow-seam з tracked-лінками
- Face-value P2P-resale (закритий контур) + іменні квитки
- Промокод-ввід на checkout (web+Android)
- Realtime чат (WS/SSE) + блокування/мьют DM
- Reviews-gate (лише відвідувачі); серверне дублювання age-gate на reserve
- Reconciliation внутрішній escrow↔провайдер; per-currency агрегація скрізь
- CI pipeline (.github/workflows); ops: /readiness, graceful shutdown, backups, метрики
- Refund-protection revenue-share на checkout; early/instant payout за +3%
- Full-text пошук + довідник категорій + cursor-пагінація

## Змінити

- payouts.status DEFAULT 'paid' → 'pending'; перехід у 'paid' лише після підтвердженого переказу
- refund/return: ставити escrow_status='refunded' ЛИШЕ після підтвердження повернення провайдером (зараз — одразу в БД)
- webhooks.rs: розширити з order_created/order_paid на refund/chargeback/dispute (зараз no-op)
- Узгодити CLAUDE.md/spec.md з реальністю auth (зараз код суперечить доктрині Google-only)
- Web/Android checkout: перестати завжди викликати pay-test; розгалужувати за CheckoutResp.mode
- tickets.rs scan: дозволити валідацію делегованому staff, не лише organizer_id==user.id
- Admin GMV-дашборд: рахувати per-currency замість змішування валют (admin.rs:32)
- /me/tickets ціна: fmtPrice замість хардкоду '₴'
- reviews.rs: gate на власника квитка події, що відбулась
- Грошові дії admin (payout mark paid/failed, web return/dispute): замінити prompt()/confirm() на accessible-модалки з підтвердженням
- discover/events/feed: додати cursor-пагінацію замість жорсткого LIMIT 50
- CSV-експорт відвідувачів: екранувати коми/лапки/переноси (anti-injection)
- JWT: перенести з localStorage у httpOnly cookie
- Категорія події: з вільного тексту → керований довідник
- KYC: показувати адміну bank-details для верифікації (зараз навмисно приховані) + завантаження документів

## Прибрати

- Google-кнопка-заглушка (тост 'скоро') в Android AuthScreen — або замінити реальним OAuth, або прибрати до реалізації
- TestPayScreen / pay-test як ЄДИНИЙ платіжний шлях у проді — лишити test-mode тільки за settings.mode==test, не як дефолт
- Мертвий слот google_sub, якщо доктрину змінено на email-first (інакше — заповнити реальним OAuth)
- next.config images.unoptimized=true — прибрати на користь оптимізації при медіа-pipeline
- Хардкод '₴' у web /me/tickets
- Свідомо відкладені, але декларовані як DONE* пункти (ticket tiers) — прибрати фіктивний DONE-статус з audit-роадмапу до реальної реалізації
- Розсинхрон: cancelled у admin-UI фільтрі подій, який PATCH не приймає (admin.rs:191) — або додати підтримку, або прибрати з UI

## Монетизація

Рекомендований мікс під UA-ринок (ефективна комісія помітно нижча за Fever 15-30%, ближче до нижнього Posh з поправкою на купівельну спроможність):

1) ОСНОВНЕ — сервісна комісія 7-10% all-in, видима покупцю однією сумою в UAH (як DICE/Posh — 'що бачиш, те й платиш', дає ~30% вищу конверсію), з якої ~5-6% чиста маржа після процесингу. Безкоштовні події — 0% комісії (максимізувати cold-start пропозиції). Дефолт виплати — post-event (HELD→...→PAID_OUT). Прозорість прямо б'є по неповоротному 10% збору Karabas/Concert.ua.

2) EARLY/INSTANT PAYOUT за +3% (як Eventbrite) — клапан проти головного болю held-funds (cash-flow організатора: майданчик/гонорари/продакшн оплачуються ДО події). Для перевірених організаторів — частковий аванс до 50% виручки.

3) FACE-VALUE RESALE комісія 5-10% — дохід + анти-спекуляція + захист бренду; закриває білу пляму всіх UA-конкурентів (зараз перепродаж і шахрайство в Telegram).

4) REFUND-PROTECTION revenue-share на checkout (Cover Genius/XCover з UA-покриттям або власний cancellation-fund) — нульовий балансовий ризик, +63% охочіше купують з CFAR, і прямо ЗНИЖУЄ chargeback-ризик held-моделі (покупець іде в страхову, не в банк).

5) PROMOTER/KICKBACK-мережа — не прямий дохід, а cold-start supply-двигун (знижує CAC), монетизується через основну комісію з квитків, які промоутери продають.

6) ПІДПИСКА-ТІРИ для організаторів (v1+, апсел не gate, як Luma Plus): Free (стандартна комісія, post-event payout) → Pro (нижча комісія, instant payout включений, буст у соц-стрічці/таргет на підписників, розширена аналітика) → Partner (негоційований, топ-промоутери). Монетизувати соц/маркетплейс-шар, якого немає в Karabas/Concert.ua.

7) ЗГОДОМ (Tixr-стиль): розширити checkout за межі квитка — мерч/апгрейди/донати (зокрема донат-зв'язок з ЗСУ як частина воєнного позиціонування) в один потік + плани оплати/waitlist на дорогі події.

Свідомо НЕ робити: двосторонній resale-збір вище номіналу (токсично для бренду 'underground'); приховані доплати за метод оплати (антипатерн Eventix); стек 'service+processing' окремо (антипатерн Eventbrite).

## Топ-ризики

- MONEY-SAFETY (критичний): payout/refund — лише статуси в БД без реального переказу; payouts.status DEFAULT 'paid' приховує невиконані виплати. Розбіжність бухгалтерії системи й реального руху грошей → подвійні/пропущені виплати, покупці бачать 'refunded' без коштів. Блокує комерційний запуск.
- СТРУКТУРНИЙ MoR-ризик (критичний): LS/Paddle AUP орієнтовані на digital SaaS, можуть забороняти event-tickets+third-party-payouts. Якщо LS відмовить — потрібен інший грошовий рейк; це може змусити переписати інтеграцію навколо escrow.rs. Валідувати ДО поглиблення escrow-логіки.
- COMPLIANCE (критичний): тримання чужих коштів (escrow) + виплати третім особам у UA = зона регулювання НБУ/money-transmission; реального KYC/AML немає, legal — TEMPLATE. Live-ескроу без юрвисновку — регуляторний і кримінально-правовий ризик.
- ДОКТРИНА vs КОД (високий): реалізовано email+password, який CLAUDE.md прямо забороняє (Google-only). Розходження docs↔код → неправильні рішення; прихована бомба при додаванні OAuth (Google-only акаунти не зможуть логінитись/видалятись через поточну re-auth паролем).
- FRAUD/ABUSE (високий): немає anti-bot/rate-limiting/risk-scoring; >80% трафіку гарячих подій — боти; self-serve онбординг без KYB — фейкові події/відмивання; DM без модерації — спам/харасмент. Платіжна платформа з ескроу без risk-контролю вразлива.
- ОРГАНІЗАТОР-ВІДТІК (високий): held-funds сприймається як 'заморозка коштів' (причина #1 відтоку від Eventbrite) + cash-flow біль; без early-payout і payout-видимості організатори підуть до direct-settlement конкурентів.
- CHARGEBACK (середній): LS refund/chargeback webhooks ігноруються; з 01.01.2026 Visa VAMP <0.5%; події = high-risk (довгий розрив оплата↔подія, friendly fraud до 60%). Без обробки — кошти можуть бути paid_out до chargeback.
- ЯКІСТЬ/ПРОЦЕС (середній): немає CI; 100% coverage недоказово; SDD для M1-M8 відсутні; за власною §11 репо не 'Done'. Ризик регресій у грошовому коді без автоматичних гейтів.
- МАСШТАБ (низько-середній): discover/feed/sweep/fan-out без пагінації/батчингу — деградація на 10k+ подій/великій базі підписників.
- БЕЗПЕКА КЛІЄНТА (середній): JWT у localStorage (XSS), UGC без явної санітизації, CSV-injection, медіа без перевірки вмісту — вектори перед публічним запуском.
- ВИКОНАВЧИЙ (середній): обсяг P0 великий (12 пунктів) для pre-launch; ризик розпорошення. Дисципліна вузького Київ-фокусу й послідовності M0→M2 критична, щоб не запустити з діркою в грошах/compliance.

## Відкриті питання для засновника

1. Auth-доктрина: лишаємось на Google-only (реалізувати OAuth, прибрати email-password) чи легалізуємо email+Google і оновлюємо CLAUDE.md/spec? Це впливає на GDPR-erasure (re-auth), rate-limiting і google_sub.
2. MoR-стратегія: чи дозволяє Lemon Squeezy продаж event-tickets + виплати організаторам? Якщо ні — UA-еквайр (LiqPay/Fondy/WayForPay) чи Stripe Connect як грошовий рейк? Це архітектурне рішення для escrow.rs.
3. Виплатний рейк організаторам: LS bank-payouts (USD-конвертація, UA-рахунки не підтверджені) чи локальний UAH-payout (monobank/IBAN)? Крос-кордонні виплати — ризик.
4. Рівень escrow-арбітражу: тримати кошти на сегрегованому trust-рахунку через ліцензованого еквайра/MoR (уникнути money-transmitter статусу) чи отримувати власну ліцензію платіжної установи НБУ? Потрібен юрвисновок.
5. Cash-flow організаторів: пропонувати early-payout (+3%) і/або частковий аванс перевіреним організаторам? Який chargeback-резерв (rolling reserve) закладати у фінмодель?
6. Рівень комісії: фіксувати 7-10% all-in для всіх чи диференціювати (нижче для перевірених/Pro-підписки)? Безкоштовні події точно 0%?
7. Anti-scalping vs gifting: наскільки жорстко прив'язувати квиток до особи (DICE-контур) — це покращує fraud/resale, але ускладнює подарунки/групи (заявлена фіча gifting). Знайти баланс.
8. KYC-провайдер: Sumsub/Onfido (глобальний) чи Diia.Підпис/Diia.Card (UA-нативний, дешевший, але лише UA-резиденти)? Поріг KYC (всі організатори vs від N виручки)?
9. Resale-політика: запускати face-value P2P-resale у v1 (закриває білу пляму) чи відкласти? Цінова стеля = строго номінал чи номінал + комісія?
10. Geo-scope: тримати spec UA/UAH/Київ чи рухатись до 'worldwide' з product-audit-бачення? Це впливає на i18n, multi-currency, FX, legal (EAA/DSA при EU-користувачах).
11. Воєнне правило рефандів: інтегрувати офіційний air-alert API як сигнал авто-вирішення спорів (диференціатор) чи лишити ручним? Юридична валідність '70-80% події = відбулась'.
12. Послідовність запуску: soft-launch на Києві з обмеженим колом організаторів (closed beta) для відпрацювання грошей/compliance, чи відкритий self-serve одразу? Перше безпечніше для P0-ризиків.
