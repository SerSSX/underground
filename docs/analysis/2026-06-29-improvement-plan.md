# Underground — План покращень, ринкове позиціонування та дорожня карта

> Deliverable #2. Стратегічний документ для засновника.
> Базис: синтезований Strategy Backbone, аудит кодової бази (`api/`, `android/`, `web/`, `admin/`, `docs/`), ринкове дослідження (глобальний + UA), governing doctrine `docs/rules.md` (v0.5.0, STRICT).
>
> **Головна теза.** Underground — це рідкісно повний для pre-launch продукт: працюючий escrow-каркас, повний соц-граф, три клієнти, зрілий back-office. Дорога до комерційного запуску — **не переписування, а замикання реального грошового транспорту, безпеки і compliance поверх уже зрілого ядра**. Технічний борг чесний і локалізований у грошовому seam (`api/src/escrow.rs`).

---

## 1. Ринковий контекст

### 1.1. Глобальні гравці

Ринок розколовся на два полюси: **commerce-OS для великих організаторів** (Eventbrite, Tixr, Universe, Eventix) і **social-first discovery для молоді/нічного життя** (Posh, Shotgun, Partiful, Luma, DICE). Underground цілиться в другий, менш зайнятий полюс — і це правильно.

| Гравець | Регіон / модель | Комісія | Ключова сила | Урок для Underground |
|---|---|---|---|---|
| **Eventbrite** | Глобальний, self-serve | ~3.7% + $1.79/квиток + 2.9% процесинг (~10-11% all-in); free = 0% | SEO/discovery, зрілий кабінет | Стек «service + processing» і заморозка коштів — причина №1 відтоку. Робити навпаки: all-in ціна + швидкий payout |
| **DICE** | UK/Global, music; куплений Fever 2025 | ~10-15% від номіналу, у видимій ціні | Anti-scalping (закритий контур, face-value resale), ~95% retention | Закритий контур квитка + ресейл за номіналом ідеально лягають на escrow Underground |
| **Fever** | Глобальний, 450+ міст | ~15-30% (Originals ~50%+) | Demand-prediction, кураторство | Збирати сигнали попиту (follow/save) до продажу; **уникати хижих 15-30%** на UA |
| **Luma** | Глобальний, tech/community | 0% free / 5% paid; Plus $69/міс = 0% | Community Calendars (підписка на організатора → пуш) | Follow-організатора з авто-пуш — головна retention-петля |
| **Partiful** | США, Gen Z | $0 (живе на VC) | Видимий guest-list, sharable cover-меми, SMS Blast, +400% YoY | «Хто йде» = доведений #1 conversion-двигун; але без квиткового доходу |
| **Posh** | США, nightlife, iOS-first | ~10% + $0.99 з покупця; 0% організатору | Promoter/Kickback-мережа, all-in ціна (~30% вища конверсія), $350M GMV | **Найближчий аналог.** Копіювати структуру; **експлуатувати iOS-first слабкість через Android-first** |
| **Shotgun** | Global, nightlife, FR | Per-ticket fees (вищі, скарги) | «Де друзі», score, ambassador, in-app resale | Гейміфікація + соц-граф друзів; не за рахунок надмірних fee |
| **Resident Advisor** | Global, electronic | Flat per-ticket | Редакційний авторитет про сцену | Легкий кураторський шар про київську сцену для органічного SEO |
| **Tixr** | Global, 60 країн, enterprise | Negotiated | Єдиний checkout (квиток+мерч+апгрейди+плани оплати) | Розширити checkout за межі квитка → +середній чек/LTV |
| **Bandsintown** | Global, 100M+ | Free (marketplace/affiliate) | Discovery за музичним смаком (Spotify/Apple/Shazam) | Персональне відкриття за смаком — whitespace, якого нема в UA |
| **Universe (Ticketmaster)** | Global | 2-2.5% + flat | Низька базова комісія | Реалістичний орієнтир «дешевого» полюса |
| **Eventix/Weeztix** | Europe | від ~€0.83 flat/квиток | White-label, flat fee | Уникати прихованих доплат за метод оплати |

**Висновки по глобальному ринку:**
1. **Product bar задають Posh + DICE + Luma:** all-in прозора ціна, mobile clean-UX, follow-організатора з авто-пуш, соц-стрічка відкриття, закритий контур з face-value resale.
2. **Прозорість комісій — норма.** Стек «service + processing» (Eventbrite) — антипатерн.
3. **Домінантна монетизація:** 0% організатору + комісія з покупця (~10-13% Posh) АБО підписка, що обнуляє fee (Luma Plus).
4. **Anti-scalping через закритий контур** — природний союзник escrow.
5. **Найсильніший retention — соц-об'єкти навколо квитка**, а не сам квиток.

### 1.2. UA-ринок

Усі інкумбенти — **класичні транзакційні каси без соц-шару, без escrow-захисту й без легального вторинного ринку**.

| Гравець | Модель | Комісія | Слабкість (= можливість Underground) |
|---|---|---|---|
| **Concert.ua** | Агрегатор з 2012, web+app | Сервісний збір ~10% з покупця, **неповоротний** | Гучні скарги на повернення за зірвані тривогами заходи; повернення промокодами, не грошима; нема соц-шару |
| **Karabas / kasa.com.ua** | Найбільша база анонсів + офлайн-каси, ~625K візитів/міс | 10% з покупця; повертається лише номінал, **збір ні** | Повернення «тільки за листом організатора» (повільно, тягар на покупця); legacy UX |
| **Internet-bilet.ua** | Агрегатор 2-го ешелону | ~10% | Слабкий бренд серед молоді; без mobile-first/соц |
| **Kontramarka.ua** | Оператор (система Maestro), B2B/майданчики | ~10% | «Старий» оператор; орієнтація на великі майданчики, не на independent/клубну сцену |
| **Ticketsbox** | Quasi-self-service + CRM для організаторів | «Мінімальний збір, інколи 0%» | Нема escrow-захисту покупця, нема соц/discovery/resale |

**Висновки по UA-ринку:**
1. **Стандарт — ~10% неповоротний збір** з покупця. Головна больова точка довіри.
2. **Повернення коштів — найслабше місце інкумбентів.** Повільно, через лист організатора, промокодами.
3. **Воєнний контекст** створив правило: тривога >2 год після проходження 70-80% заходу = подію вважають проведеною (форс-мажор). Це можна **кодифікувати в escrow** як авто-вирішення спорів.
4. **Платіжні рейки UA дешеві й зрілі:** monobank 1.3% (укр. картки) / 2% (іноземні); WayForPay ~2.5%; LiqPay ~2.75%; Apple/Google Pay — must.
5. **Lemon Squeezy виплачує у USD** — UAH-виплати прямо не підтверджені; для крос-кордонних виплат організаторам це ризик.
6. **Жоден інкумбент** не має соц/discovery-шару, легального resale, прозорого авто-повернення прив'язаного до факту події. **Це біла пляма.**

### 1.3. Джерела

**Глобальні / монетизація:** eventbrite.com/organizer/pricing, checkoutpage.com/blog/eventbrite-fees, en.wikipedia.org/wiki/Dice_(ticketing_company), variety.com/2025/music/global/dice-ceo-phil-hutcheon..., wearerockwater.com/fever-buys-dice, luma.com/pricing, cnbc.com/2025/04/19/meet-partiful..., en.wikipedia.org/wiki/Partiful, posh.vip, support.posh.vip/en/articles/11459046-all-in-pricing, fortune.com/2026/03/19/exclusive-posh-lands-37m-series-b, techcrunch.com/2024/07/23/event-startup-posh-raises-22m, support.ra.co/article/177-service-fees, shotgun.live/en, newsroom.feverup.com/.../250714-fever-secures-100m, creators.tixr.com/products/platform, musicbusinessworldwide.com/.../bandsintown..., universe.com/pricing, eventix.io/pricing.

**MoR / escrow / chargeback:** docs.lemonsqueezy.com/help/getting-started/fees, .../marketplace, .../prohibited-products, .../getting-paid, .../payments/currencies, lemonsqueezy.com/blog/new-bank-payouts, paddle.com/help/start/intro-to-paddle/what-am-i-not-allowed-to-sell, mofo.com/.../250707-the-ftc-gives-the-merchant-of-record-model-a-paddling, fincen.gov/.../application-money-services-business-1, dfpi.ca.gov/.../escrow-trust-account-funds-not-stored-value-under-mta, happenow.app/blog/why-does-ticketmaster-pay-you-after-the-event, ticketleap.com/blog/chargebacks, chargebacks911.com/ticketing-chargebacks, covergenius.com/live-event-ticketing-protection, ravelin.com/blog/visa-vamp-changes-chargeback-disputes, beaconpayments.com/blog/visa-mastercard-chargeback-rule-changes-2025-2026.

**UA-ринок / платежі / legal:** karabas.com/offerta, kasa.com.ua/offerta, concert.ua/en/page/refund, concert.ua/en/page/offer, glavcom.ua/.../povernit-hroshi-za-kvitki..., consumerhm.gov.ua/3485-yak-povernuti-koshti-za-kvitok-na-skasovanij-kontsert, monobank.ua/en/acquiring, liqpay.ua/en/tariffs, olimpdigital.com/blog/ecommerce/top-ukraine-payment-providers, monobank.ua/en/diia-card, cms-lawnow.com/.../ukraine-adopts-use-of-digital-id-wallets, voanews.com/.../ukraine-largest-music-festival-returns, ticketsbox.com/en, similarweb.com/website/concert.ua/competitors.

**Trust/safety/fraud:** queue-it.com/blog/ticket-bots, datadome.co/.../ticket-bots, iclg.com/.../data-protection-laws.../ukraine, gls-law.company/en/new-personal-data-law-in-ukraine, sumsub.com/blog/kyb-guide, digital-strategy.ec.europa.eu/en/policies/eu-age-verification, gtlaw.com/.../european-accessibility-act-compliance, lexisnexis.com/.../court-of-justice-confirms-cancellation-rights, avellum.com/ukraine-introduces-new-law-on-consumer-protection.

**Growth:** consumerapplab.substack.com/p/virality-isnt-retention, nogood.io/blog/partiful-marketing-strategy, help.luma.com/p/luma-vs-meetup, support.posh.vip/.../kickback-affiliate-offers, reforge.com/guides/beat-the-cold-start-problem-in-a-marketplace, ticketfairy.com/blog/turning-fans-into-ambassadors.

---

## 2. Позиціонування та диференціація

> **Позиціонування: «Соціальна афіша Києва, де гроші захищені до події».**

Underground — Kyiv-first, mobile-native (Android-first) платформа, що зрощує три речі, які на UA-ринку живуть окремо: **(1) персоналізоване discovery, (2) квитковий сервіс з e-ticket/QR, (3) соціальний шар** (профілі, підписки, стрічка, відгуки, чат). Стрижнева відмінність — **escrow-маркетплейс**: покупець платить → гроші HELD → виплата організатору лише ПІСЛЯ події (`HELD→CLEARING→CLEARED→PAID_OUT`, гілки `DISPUTED`/`REFUNDED`).

### Три вектори диференціації проти UA-кас

**1. ДОВІРА через escrow** (whitespace, який не комунікує жоден UA- і майже жоден глобальний гравець):
- Гроші HELD до завершення події; скасування = автоматичне повне повернення, **включно з сервісним збором** — прямий удар по неповоротному 10% Karabas/Concert.ua.
- Кодифікувати воєнне правило (тривога >2 год після проходження 70-80% події = подія відбулась) у escrow як авто-вирішення спорів через air-alert фід — фічі немає ні в кого.
- **Критичне застереження:** escrow не має перетворитись на «заморозку коштів організатора» (причина №1 відтоку від Eventbrite) — потрібен передбачуваний швидкий payout після `CLEARED` + платний early-payout як клапан cash-flow.

**2. СОЦІАЛЬНІСТЬ як канал росту** (whitespace на UA-ринку):
- Follow-організатора з авто-пуш про нові події (Luma Community Calendars).
- «Хто з твоїх друзів іде» на сторінці події (Partiful — доведений #1 conversion-двигун).
- Стрічка активності підписок, відгуки. Знижує CAC проти платного трафіку.

**3. ЧЕСНА ЕКОНОМІКА:**
- All-in прозора ціна в UAH (одна сума, без стеку «service + processing» — як DICE/Posh/Luma, ~30% вища конверсія).
- Ефективна комісія помітно нижча за Fever 15-30%, ближче до нижнього Posh з поправкою на UA-купівельну спроможність.
- Face-value resale (стеля = номінал) як анти-скальпінг і захист бренду.

### Структурні переваги виконання
- **Android-first** проти iOS-first Posh — на UA-ринку домінує Android (пряма дистрибуційна перевага).
- **Вузький Київ-фокус** дає density/ліквідність.
- Бренд **«underground»** природно лягає на кураторський шар про київську сцену (RA-стиль) для органічного SEO.
- **Адаптований воєнний формат** (укриття, ранній старт, донат-зв'язок з ЗСУ) як частина ціннісної пропозиції.

---

## 3. GAP-аналіз: що треба для САМОСТІЙНОЇ комерційної платформи

Легенда пріоритетів: **P0** = блокер комерційного запуску; **P1** = критично для повноти/довіри; **P2** = масштаб/поліш.

### 3.1. Платежі / Ескроу / Виплати

| # | Пріор. | Область | Проблема | Що зробити |
|---|---|---|---|---|
| 1 | **P0** | Реальний payout | Payout не викликає банк/LS API — лише пише рядок і ставить `paid_out`; `payouts.status DEFAULT 'paid'` (міграція 0018) позначає виплату оплаченою до фактичного переказу. Гроші переказує людина поза системою | Інтегрувати реальний виплатний рейк (monobank/IBAN UAH або Stripe Connect transfers) за абстракцією `escrow.rs`; виправити default на `'pending'`; перехід у `'paid'` лише після підтвердженого переказу |
| 2 | **P0** | Реальний refund | `refund/return` лише ставить `escrow_status='refunded'` у БД (`tickets.rs:239`, `admin.rs:315`) — покупцю гроші НЕ повертаються | Підключити LS/еквайр refund API; `'refunded'` лише після підтвердження провайдером |
| 3 | **P0** | Live-платежі E2E | Web `confirmBuy` завжди викликає `pay-test`; `CheckoutResp` без `checkout_url`; Android завжди веде в `TestPayScreen`. У live купівля зламається | Прокинути `checkout_url` у `CheckoutResp`; web-редірект на LS hosted-checkout + success/return; Android WebView/Custom Tab з перевіркою `mode`; E2E smoke проти реального LS |
| 4 | **P0** | MoR-сумісність (структурний) | LS/Paddle AUP орієнтовані на digital SaaS; Paddle прямо забороняє event tickets+маркетплейс, LS — digital-only/no third-party marketplace | НЕГАЙНО письмово валідувати з LS/Stripe дозвіл на (а) event-tickets, (б) виплати третім особам. План Б: UA-еквайр (LiqPay/Fondy/WayForPay) або Stripe Connect. Не хардкодити LS у грошову логіку |
| 5 | **P0** | Chargeback/dispute webhooks | LS refund/chargeback/dispute ігноруються (`webhooks.rs:54` — лише `order_created/order_paid`). При chargeback гроші можуть бути cleared/paid_out | Обробляти `order_refunded/dispute/chargeback` з авто-freeze; persistent webhook-event ledger з idempotency-key + replay/timestamp захистом |
| 6 | **P1** | Reconciliation | Немає звірки внутрішнього escrow-стану з реальними балансами провайдера | Механізм reconciliation; per-currency агрегація; immutable ledger руху коштів (double-entry), а не лише мутабельна `orders.escrow_status` |
| 7 | **P2** | Multi-currency | Дашборд GMV змішує валюти (`admin.rs:32`); ціна в `/me/tickets` захардкоджена в ₴ | Per-currency агрегація скрізь; `fmtPrice` замість хардкоду; FX-політика |

### 3.2. Квитки: купити / подарувати / повернути

| # | Пріор. | Проблема | Що зробити |
|---|---|---|---|
| 8 | **P1** | Лише один `price+tickets_total`; немає ticket tiers (VIP/early-bird/звичайний) | Ticket tiers з per-tier capacity/ціною/вікном продажу; узгодити з escrow/refund |
| 9 | **P1** | Промокод організатор створює, але покупець на web не може застосувати (`?promo` не передається) | Поле вводу промокоду на сторінці події + прокидання у checkout на обох клієнтах |
| 10 | **P1** | Немає легального вторинного ринку; перепродаж іде в Telegram (з шахрайством) | Закритий P2P-resale за номіналом (стеля=ціна), квиток повертається в систему й перевипускається, комісія 5-10% |
| 11 | **P1** | Квиток (gift/transfer) передається вільно — вектор скальпінгу | Іменні квитки прив'язані до акаунта; transfer лише через систему; resale лише face-value через waitlist |
| 12 | **P2** | Немає recurring/multi-session/seating/waitlist | Додати поетапно (waitlist при sold-out — пріоритет; seating — пізніше для залів) |

> **Що працює добре (зберегти):** реальний QR (zxing), reserve free-квитка з dedup, transfer (gift за email) з age-gate+frozen-block, self-return з escrow-рефандом, buyer-dispute→frozen. Зрілість квитків ~80%.

### 3.3. Валідація на вході + делегування персоналу

| # | Пріор. | Проблема | Що зробити |
|---|---|---|---|
| 13 | **P0** | Сканувати квиток може ЛИШЕ організатор події (`tickets.rs:179`); немає ролі ORGANIZER/STAFF/delegation. Door-staff мусить ділитися повним акаунтом — небезпечно й блокує операційний сценарій | Модель `team/staff` з делегованим scoped-правом (validate-only на конкретні події організатора), без доступу до фінансів/редагування. UI делегування + door-scan на мобільному. Дозволити валідацію делегованому staff (`tickets.rs scan`) |

### 3.4. Організаторська адмінка / кабінет

| # | Пріор. | Проблема | Що зробити |
|---|---|---|---|
| 14 | **P1** | Організатор не бачить стану виплат (HELD→...→PAID_OUT); web payout-history read-only; на мобільному немає аналітики — «летить наосліп» | Organizer-scoped payout-history endpoint+UI (web+Android); продажна аналітика (продано/виручка/відвідувачі/escrow-розбивка) на мобільному |
| 15 | **P1** | Dispute = лише `escrow_status='frozen'` + ручний admin-toggle. Немає черги спорів, причин/доказів, дедлайнів, окремих результатів, нотифікацій сторін | Dispute-модель: причина/докази, SLA-дедлайни, статуси рішення, двостороння комунікація, авто-нотифікації; air-alert авто-вирішення |
| 16 | **P2** | Грошові admin-дії на `prompt()/confirm()`; CSV-експорт без екранування (injection); фільтр `cancelled` у UI, який PATCH не приймає | Accessible-модалки підтвердження; екранування CSV; узгодити фільтр з PATCH |

> **Сильна сторона:** admin — найзріліша площина (~62%): дашборд GMV/комісія, модерація, черга скарг з enforcement, ручний escrow refund/freeze/release, payout ledger з KYC-gate, audit-trail.

### 3.5. Соцмережа: стрічка підписок + коментарі + відгуки

| # | Пріор. | Проблема | Що зробити |
|---|---|---|---|
| 17 | **P1** | Reviews можна писати без відвідування (`reviews.rs`) — накрутка рейтингу | Gate: відгук лише власнику квитка на подію, що відбулась |
| 18 | **P1** | Чат лише polling (4с), без блокування/мьюту; будь-хто пише будь-кому — спам/харасмент | WS/SSE для realtime; блокування/мьют; rate-limit DM |
| 19 | **P1** | Growth-петлі не активовані: немає видимого guest-list «хто йде», follow-push не доставляється (FCM-транспорт TODO `main.rs:139`) | «Хто з друзів іде» + guest-list на сторінці події; замкнути FCM HTTP-транспорт; promoter/kickback-мережа на escrow-seam |
| 20 | **P2** | Стрічка/списки без пагінації (LIMIT 50); fan-out нотифікацій без батчингу; пости без медіа | Cursor-пагінація; батчинг fan-out; медіа в постах |

> **Сильна сторона:** повний соц-граф (~78%): ідемпотентні follow/unfollow, feed = пости підписок ∪ події за інтересами з `reason`, лайки/коментарі, відгуки 1..5 (upsert), DM-чат.

### 3.6. Мобільний застосунок (Android)

| # | Пріор. | Проблема | Що зробити |
|---|---|---|---|
| 21 | **P0** | Оплата ЛИШЕ test-mode (`TestPayScreen`); `CheckoutResp.mode` не перевіряється; немає LS WebView. У проді платний квиток продати неможливо | Розгалужувати за `CheckoutResp.mode`; WebView/Custom Tab на live checkout_url |
| 22 | **P0** | Google-кнопка = тост-заглушка «скоро» (`AuthScreen.kt:113`), що суперечить доктрині | Реалізувати Google OAuth АБО прибрати кнопку до реалізації (див. open_questions) |
| 23 | **P1** | Немає door-staff делегування на мобільному; немає organizer-аналітики/payout-видимості | Door-scan делегування; organizer-аналітика на мобільному (див. #13, #14) |
| 24 | **P2** | Deep-link на `ticket/{id}` без попереднього fetch → «не знайдено»; ViewModel ковтають винятки тихо | Fetch by id; видимі помилки |

> **Сильна сторона:** широкий Compose-клієнт (~72%, dark-only, 5 табів): онбординг, афіша+discovery, e-ticket+QR, gift/return/dispute, feed/пости/коментарі, профілі, чат, нотифікації, створення/керування подіями, scan.

### 3.7. Trust & Safety / KYC / Legal / Ops

| # | Пріор. | Проблема | Що зробити |
|---|---|---|---|
| 25 | **P0** | Auth: реалізовано email+password, тоді як CLAUDE.md/spec оголошують Google OAuth ЄДИНИМ методом (прямий конфлікт); немає `/api/auth/google`, `google_sub` завжди NULL | Засновник вирішує доктрину. Якщо Google-only: `POST /api/auth/google` з верифікацією ID-token + лінкування за email. Інакше: оновити spec/CLAUDE.md + додати rate-limit на email-login |
| 26 | **P0** | KYC/AML: loose-IBAN + ручне admin-verify наосліп (адмін не бачить bank-details); немає документів/Sumsub/Onfido/AML | Інтегрувати Sumsub/Onfido (або Diia.Підпис/Diia.Card); mod-97 IBAN; KYB для організаторів (ЄДРПОУ/ІПН/бенефіціари); KYB-gate на `CLEARED→PAID_OUT` |
| 27 | **P0** | Anti-fraud/rate-limiting: немає velocity/risk-scoring/blacklist; немає rate-limit (логін `BR-017`, checkout); немає anti-bot/черги | Rate-limiting (`governor`) на auth/checkout; device-fingerprinting; ліміт квитків на акаунт; черга на сплески; velocity-чеки; 3DS/SCA |
| 28 | **P0** | Legal: усі політики TEMPLATE без counsel-review; escrow/held-funds — зона НБУ; GDPR-аналог УЗ №8153 (штраф до 5% обороту) | UA-юрвисновок (НБУ/escrow-арбітраж) ПЕРЕД live-ескроу; counsel-review усіх політик; межа відповідальності LS↔Underground |
| 29 | **P0** | CI/CD: немає `.github/workflows`; заявлені 100% coverage недоказові; за §11 репо не «Done» | CI pipeline (lint+test+coverage-gate+security-audit), fail-fast як `scripts/ci.sh` |
| 30 | **P0** | Ops: немає graceful shutdown, backups, `/readiness`, метрик; інцидент БД може пошкодити escrow | `/readiness`+`/health`, graceful shutdown, автобекапи, метрики (latency/errors/escrow-sweep), алерти на застряглі кошти |
| 31 | **P1** | Age-assurance: self-declared `dob` без верифікації; DSA/UK OSA забороняють чекбокс для restricted | Реальний age-assurance для restricted-подій (age-estimation/ID/Diia); серверне дублювання age-gate на reserve |
| 32 | **P2** | JWT у localStorage (XSS); UGC без санітизації; медіа content-type від клієнта (підробляється) | httpOnly cookie; санітизація UGC; CDN+resize+антивірус/модерація завантажень |
| 33 | **P2** | Accessibility: dark-only тема має проходити contrast-ratio; TalkBack/focus не перевірені (EAA при EU-амбіціях) | WCAG 2.1 AA / EN 301 549 аудит Android+web |
| 34 | **P2** | Процес: SDD для M1-M8 відсутні; роадмап лише в `audit/`; нема ui-schematics/baselines/design-tokens; M9 Figma BLOCKED | Ретроспективно TASK+SDD; §11 артефакти; розблокувати M9 |

---

## 4. Що ДОДАТИ / ЗМІНИТИ / ПРИБРАТИ

### 4.1. ДОДАТИ
- `POST /api/auth/google` з верифікацією ID-token + лінкування за email (якщо доктрина Google-only).
- Реальний payout-транспорт (monobank/IBAN UAH або Stripe Connect) за абстракцією `escrow.rs`.
- Реальний refund через провайдера (LS/еквайр API).
- `checkout_url` у `CheckoutResp` + live hosted-checkout (web redirect, Android WebView).
- Обробка LS refund/chargeback/dispute webhooks з авто-freeze + persistent webhook-event ledger.
- Модель ролей `team/staff` з делегованим scoped door-scan правом.
- Реальний KYC/KYB (Sumsub/Onfido/Diia) + mod-97 IBAN + KYB-gate на payout.
- Rate-limiting (`governor`) на auth+checkout; 3DS/SCA; velocity/risk-scoring.
- Ticket tiers (VIP/early-bird/звичайний) з per-tier capacity/ціною/вікном.
- Dispute-resolution workflow (причини/докази/SLA/результати/нотифікації) + air-alert авто-вирішення.
- Organizer payout-history endpoint+UI та продажна аналітика на мобільному.
- «Хто з друзів іде» / видимий guest-list на сторінці події.
- Follow-організатора push (замкнути FCM HTTP-транспорт у `main.rs`).
- Promoter/Kickback-мережа на escrow-seam з tracked-лінками.
- Face-value P2P-resale (закритий контур) + іменні квитки.
- Промокод-ввід на checkout (web+Android).
- Realtime чат (WS/SSE) + блокування/мьют DM.
- Reviews-gate (лише відвідувачі); серверне дублювання age-gate на reserve.
- Reconciliation внутрішній escrow↔провайдер; per-currency агрегація скрізь.
- CI pipeline; ops: `/readiness`, graceful shutdown, backups, метрики.
- Refund-protection revenue-share на checkout; early/instant payout за +3%.
- Full-text пошук + довідник категорій + cursor-пагінація.

### 4.2. ЗМІНИТИ
- `payouts.status DEFAULT 'paid'` → `'pending'`; перехід у `'paid'` лише після підтвердженого переказу.
- `refund/return`: ставити `escrow_status='refunded'` ЛИШЕ після підтвердження провайдером (зараз — одразу в БД).
- `webhooks.rs`: розширити з `order_created/order_paid` на refund/chargeback/dispute (зараз no-op).
- Узгодити CLAUDE.md/spec.md з реальністю auth.
- Web/Android checkout: розгалужувати за `CheckoutResp.mode`, не завжди `pay-test`.
- `tickets.rs scan`: дозволити валідацію делегованому staff.
- Admin GMV-дашборд: per-currency замість змішування валют.
- `/me/tickets` ціна: `fmtPrice` замість хардкоду `'₴'`.
- `reviews.rs`: gate на власника квитка події, що відбулась.
- Грошові admin-дії: `prompt()/confirm()` → accessible-модалки.
- `discover/events/feed`: cursor-пагінація замість LIMIT 50.
- CSV-експорт: екранування ком/лапок/переносів.
- JWT: localStorage → httpOnly cookie.
- Категорія події: вільний текст → керований довідник.
- KYC: показувати адміну bank-details для верифікації + завантаження документів.

### 4.3. ПРИБРАТИ
- Google-кнопку-заглушку (тост «скоро») в Android — замінити OAuth або прибрати.
- `TestPayScreen`/`pay-test` як ЄДИНИЙ шлях у проді — лишити лише за `mode==test`.
- Мертвий слот `google_sub`, якщо доктрину змінено на email-first (інакше — заповнити OAuth).
- `next.config images.unoptimized=true` — прибрати при медіа-pipeline.
- Хардкод `'₴'` у web `/me/tickets`.
- Фіктивний DONE-статус ticket tiers з audit-роадмапу до реальної реалізації.
- Розсинхрон `cancelled` у admin-UI фільтрі, який PATCH не приймає.

---

## 5. Монетизація (модель для UA, з урахуванням escrow + MoR)

> Принцип: ефективна комісія помітно нижча за Fever 15-30%, ближче до нижнього Posh з поправкою на UA-купівельну спроможність. **All-in прозора ціна в UAH — не опція, а вимога** (~30% вища конверсія).

### 5.1. Рекомендований мікс

| # | Потік | Параметри | Обґрунтування |
|---|---|---|---|
| 1 | **Сервісна комісія (основне)** | 7-10% all-in, видима покупцю однією сумою в UAH; ~5-6% чиста маржа після процесингу. **Безкоштовні події — 0%** | Прозорість прямо б'є по неповоротному 10% Karabas/Concert. 0% на free максимізує cold-start пропозиції |
| 2 | **Early/Instant payout** | +3% (як Eventbrite); для перевірених — частковий аванс до 50% виручки | Клапан проти головного болю held-funds (cash-flow: майданчик/гонорари/продакшн оплачуються ДО події) |
| 3 | **Face-value resale** | Комісія 5-10%, стеля = номінал | Дохід + анти-спекуляція + захист бренду; закриває білу пляму всіх UA-конкурентів |
| 4 | **Refund-protection** | Revenue-share на checkout (Cover Genius/XCover з UA-покриттям або власний cancellation-fund) | Нульовий балансовий ризик; +63% охочіше купують з CFAR; **прямо знижує chargeback-ризик** (покупець іде в страхову, не в банк) |
| 5 | **Promoter/Kickback** | Не прямий дохід — cold-start supply-двигун; tracked-лінки, розщеплення комісії в escrow | Знижує CAC; монетизується через основну комісію з проданих квитків |
| 6 | **Підписка-тіри (v1+)** | Free (стандарт, post-event) → Pro (нижча комісія, instant payout, буст у стрічці, аналітика) → Partner (негоційований) | Апсел, не gate (як Luma Plus). Монетизує соц/маркетплейс-шар, якого нема в Karabas/Concert |
| 7 | **Розширений checkout (згодом)** | Мерч/апгрейди/донати (зокрема ЗСУ) в один потік + плани оплати/waitlist | Tixr-стиль: +середній чек/LTV; донат-зв'язок з ЗСУ як частина воєнного позиціонування |

**Дефолт виплати — post-event** (`HELD→...→PAID_OUT`).

### 5.2. Свідомо НЕ робити
- Двосторонній resale-збір вище номіналу (токсично для бренду «underground», StubHub/Viagogo).
- Приховані доплати за метод оплати (антипатерн Eventix).
- Стек «service + processing» окремо (антипатерн Eventbrite).

### 5.3. Ризики монетизації
- **MoR-несумісність:** LS/Paddle можуть забороняти event-tickets+third-party-payouts → потрібен інший грошовий рейк (UA-еквайр/Stripe Connect). **Валідувати ДО поглиблення escrow.**
- **Held-funds як «заморозка»:** без early-payout і payout-видимості організатори підуть до direct-settlement конкурентів.
- **Chargeback:** з 01.01.2026 Visa VAMP <0.5%; події = high-risk (дружній fraud до 60%). LS бере $15/dispute — з'їдає маржу при сплеску.
- **Регуляторка escrow:** тримання чужих коштів = зона НБУ; rolling reserve треба закласти у фінмодель.

---

## 6. Дорожня карта (з повагою до `docs/rules.md`)

> **Процесна вимога (§0/§3/§12, STRICT):** для КОЖНОЇ задачі — `docs/backlog/TASK-###-*.md` (lifecycle INTAKE→REFINED→ESTIMATED→READY) ПЕРЕД кодом, і `docs/specks/SDD-###-*.md` з `AC-1..n` + **`SEC-1..n`** + Test Matrix. Speck-audit перед кодом. Кожна сюїта — positive + negative + **[SEC]**. Без нових high/critical (§9.1). 100% unit+integration (§6).
>
> Нижче для кожного майлстоуна вказано **що йде в backlog/specks**.

### M0 — Process & truth reconciliation
**Ціль:** привести docs у відповідність до коду й розблокувати дисципліну rules.md перед будь-яким новим грошовим кодом.

**Пункти:**
- Вирішити доктрину Auth (Google-only vs email+Google) і синхронізувати CLAUDE.md/spec.md з реальністю.
- Завести TASK-### + SDD-### для вже реалізованих M1-M8 (ретроспективно) і для всіх P0 нижче.
- Створити §11 артефакти: `docs/ui-schematics/`, `docs/ui-baselines/`, `docs/ui-design-tokens.md`.
- Підняти CI (`.github/workflows`): lint+test+coverage-gate+security-audit, fail-fast.
- Письмово валідувати MoR-сумісність з LS/Stripe.

**Backlog/Specks:** `TASK-AUTH-doctrine`, `TASK-CI-pipeline`, `TASK-MoR-validation`, `TASK-retro-SDD-M1..M8`; `SDD-CI`, `SDD-auth-doctrine`.
**Чому:** rules.md робить backlog+speck передумовою коду; MoR-валідація — архітектурний блокер, що може змінити весь грошовий seam. Без цього подальша робота порушує доктрину й ризикує переробкою.

### M1 — Real money seam (escrow → provider)
**Ціль:** замкнути фактичний рух грошей: live-checkout, реальні payout і refund, chargeback-обробка.

**Пункти:**
- `checkout_url` у `CheckoutResp`; web-редірект на LS hosted-checkout + success/return; Android WebView/Custom Tab з перевіркою `mode`.
- Реальний payout-рейк (UA monobank/IBAN UAH або Stripe Connect) за `escrow.rs`; `payouts.status` default→`'pending'`.
- Реальний refund через провайдера; `'refunded'` лише після підтвердження.
- LS refund/chargeback/dispute webhooks з авто-freeze; persistent webhook-event ledger + idempotency-key + replay-захист.
- E2E smoke проти реального LS + reconciliation внутрішній стан↔провайдер.

**Backlog/Specks:** `TASK-live-checkout`, `TASK-real-payout`, `TASK-real-refund`, `TASK-webhook-chargeback`, `TASK-reconciliation`; **SDD з SEC-векторами на кожну** (idempotency, replay, signature, race на payout, негативні [SEC] вектори), 100% unit+integration.
**Чому:** без реальних грошей продукт не комерційний (P0). Найбільший залишковий ризик («real-money correctness can't be proven in CI»). Escrow вже існує — потрібно лише замкнути транспорт, тому ROI найвищий.

### M2 — Trust, safety & compliance baseline
**Ціль:** non-negotiable речі платіжної платформи: KYC/KYB, anti-fraud, age, ops, legal.

**Пункти:**
- Реальний KYC/KYB (Sumsub/Onfido або Diia); mod-97 IBAN; KYB-gate на `CLEARED→PAID_OUT`.
- Rate-limiting (`governor`) на auth+checkout; 3DS/SCA; velocity/risk-scoring на замовлення й виплати.
- Реальний age-assurance для 18+/21+; серверне дублювання age-gate на reserve.
- Ops: `/readiness`, graceful shutdown, backups, метрики, алерти на застряглі кошти.
- UA-юрвисновок (НБУ/escrow-арбітраж) + counsel-review усіх політик; межа відповідальності LS↔Underground.

**Backlog/Specks:** `TASK-kyc-kyb`, `TASK-rate-limit`, `TASK-age-assurance`, `TASK-ops-foundation`, `TASK-legal-counsel`; SDD з SEC-фокусом (brute-force, bot-velocity, AML-моніторинг, escrow-state audit-log).
**Чому:** escrow + виплати третім особам = money-transmission/AML-зона; self-serve онбординг — головний вектор фроду. Передумова легального live-запуску (P0), не пост-фактум.

### M3 — Organizer operations completeness
**Ціль:** зробити організатора повноцінним: делегування door-staff, payout-видимість, ticket tiers, dispute-resolution.

**Пункти:**
- Модель `team/staff` з делегованим scoped validate-only правом; door-scan делегування на мобільному.
- Organizer payout-history endpoint+UI (web+Android) + продажна аналітика на мобільному.
- Ticket tiers (per-tier capacity/ціна/вікно), узгоджені з escrow/refund.
- Dispute-resolution workflow: причини/докази/SLA/результати/нотифікації; air-alert авто-вирішення.
- Промокоди на checkout (web+Android).

**Backlog/Specks:** `TASK-staff-delegation`, `TASK-organizer-payout-ui`, `TASK-ticket-tiers`, `TASK-dispute-workflow`, `TASK-promo-checkout`; SDD з SEC (scoped-RBAC, privilege-escalation вектори, dispute-evidence доступ).
**Чому:** засновницька візія прямо вимагає door-staff делегування й нормальної адмінки; ticket tiers критичні для платних івентів реального масштабу; без payout-видимості організатор «летить наосліп».

### M4 — Social growth loops & discovery
**Ціль:** активувати соц-first зростання — головну диференціацію проти UA-кас.

**Пункти:**
- «Хто з друзів іде» + видимий guest-list на сторінці події (доведений #1 conversion-двигун).
- Follow-організатора з реальним FCM push (замкнути транспорт у `main.rs`).
- Promoter/Kickback-мережа на escrow-seam: tracked-лінки, розщеплення комісії.
- Realtime чат (WS/SSE) + блокування/мьют; reviews лише для відвідувачів.
- Full-text пошук + довідник категорій + cursor-пагінація discovery/feed.

**Backlog/Specks:** `TASK-guest-list`, `TASK-fcm-transport`, `TASK-promoter-network`, `TASK-realtime-chat`, `TASK-reviews-gate`, `TASK-search-pagination`; SDD з SEC (privacy guest-list visibility, DM-abuse, kickback-fraud).
**Чому:** соц-граф побудований, але growth-петлі не активовані — найдешевший канал росту проти платного трафіку й whitespace на UA-ринку. FCM — остання миля для реальних нотифікацій.

### M5 — Differentiation & scale polish
**Ціль:** закрити білі плями ринку й підготувати масштаб.

**Пункти:**
- Face-value P2P-resale (закритий контур, стеля=номінал, комісія 5-10%) + іменні квитки.
- Refund-protection revenue-share (Cover Genius/XCover або власний cancellation-fund).
- Early/instant payout за +3%.
- Кураторський/редакційний шар про київську сцену (RA-стиль); JSON-LD/hreflang.
- Медіа-pipeline (CDN/resize/scan); accessibility WCAG 2.1 AA; multi-currency узгодженість; повна i18n.
- Розблокувати M9 Figma-звірку (44 фрейми).

**Backlog/Specks:** `TASK-resale`, `TASK-refund-protection`, `TASK-early-payout`, `TASK-editorial`, `TASK-media-pipeline`, `TASK-a11y`, `TASK-figma-reconcile`; SDD з SEC (resale-fraud, media-upload scan, payout-velocity).
**Чому:** resale і refund-protection — найбезпечніші додаткові доходи + захист бренду; early-payout знімає головну причину відтоку від held-funds. Монетизаційний і масштабний шар поверх working ядра.

### Зведена послідовність

| Майлстоун | Фокус | Блокує запуск? |
|---|---|---|
| M0 | Process & truth reconciliation | Так (доктрина процесу + MoR) |
| M1 | Real money seam | **Так (P0 — гроші)** |
| M2 | Trust/safety/compliance | **Так (P0 — legal/fraud)** |
| M3 | Organizer completeness | Частково (довіра організатора) |
| M4 | Social growth loops | Ні (зростання) |
| M5 | Differentiation & scale | Ні (поліш/масштаб) |

---

## 7. Топ-ризики та зниження

| Ризик | Рівень | Опис | Зниження |
|---|---|---|---|
| **Money-safety** | Критичний | Payout/refund — лише статуси в БД без реального переказу; `DEFAULT 'paid'` приховує невиконані виплати → подвійні/пропущені виплати, «refunded» без коштів | M1: реальний транспорт + `'pending'`-default + reconciliation + immutable ledger |
| **MoR структурний** | Критичний | LS/Paddle AUP можуть забороняти event-tickets+third-party-payouts | M0: письмова валідація ДО escrow-поглиблення; План Б (UA-еквайр/Stripe Connect); `escrow.rs` як єдина абстракція |
| **Compliance** | Критичний | Тримання чужих коштів = зона НБУ; KYC/AML немає; legal — TEMPLATE | M2: UA-юрвисновок ПЕРЕД live; KYC/KYB; counsel-review; сегрегований trust-рахунок через ліцензованого еквайра/MoR |
| **Доктрина vs код (Auth)** | Високий | Email+password суперечить Google-only; прихована бомба при додаванні OAuth (GDPR re-auth) | M0: рішення засновника + синхронізація docs; rate-limit |
| **Fraud/abuse** | Високий | Немає anti-bot/rate-limit/risk-scoring; >80% трафіку гарячих подій — боти; DM без модерації | M2: rate-limit, velocity, device-fingerprint, ліміт квитків, черга; M4: DM-блокування |
| **Відтік організаторів** | Високий | Held-funds = «заморозка» (причина №1 відтоку від Eventbrite) + cash-flow біль | M3/M5: early-payout, payout-видимість, прозорий таймлайн; комунікувати escrow як захист покупця |
| **Chargeback** | Середній | LS webhooks ігноруються; Visa VAMP <0.5% з 01.01.2026; події high-risk | M1: webhook-обробка + авто-freeze; M5: refund-protection; 3DS/SCA; rolling reserve |
| **Якість/процес** | Середній | Немає CI; coverage недоказово; SDD M1-M8 відсутні | M0: CI pipeline + ретро-SDD |
| **Безпека клієнта** | Середній | JWT у localStorage, UGC без санітизації, CSV-injection, медіа без перевірки | M5: httpOnly cookie, санітизація, екранування, media-scan |
| **Виконавчий** | Середній | 12 P0-пунктів для pre-launch — ризик розпорошення | Дисципліна вузького Київ-фокусу + послідовності M0→M2; closed beta |

---

## 8. Відкриті питання для засновника

> Формат: **питання → варіанти → рекомендація.**

**1. Auth-доктрина.**
Лишаємось на Google-only (реалізувати OAuth, прибрати email-password) чи легалізуємо email+Google і оновлюємо CLAUDE.md/spec?
- А) Google-only: чистий онбординг 1-тап, простіше KYC; але блокує тих, хто без Google; ламає GDPR-erasure re-auth.
- Б) Email+Google: гнучкість, але два кодові шляхи.
- **Рекомендація: Б на старті** — Google OAuth як основний 1-тап шлях (UA-перевага), email+password як fallback з rate-limit. Оновити CLAUDE.md/spec, щоб код не суперечив доктрині.

**2. MoR-стратегія.**
Чи дозволяє Lemon Squeezy event-tickets + виплати організаторам? Якщо ні — UA-еквайр (LiqPay/Fondy/WayForPay) чи Stripe Connect?
- **Рекомендація:** негайно отримати письмове підтвердження від LS. Паралельно проектувати `escrow.rs` провайдер-агностично. **План Б за замовчуванням — Stripe Connect** (escrow через held balance + transfers до connected accounts) з UA-еквайром як локальним payout-рейком.

**3. Виплатний рейк організаторам.**
LS bank-payouts (USD-конвертація, UA-рахунки не підтверджені) чи локальний UAH (monobank/IBAN)?
- **Рекомендація: локальний UAH (monobank/IBAN)** для UA-організаторів — уникає крос-кордонних ризиків і конвертації; LS лишити для процесингу покупця, якщо AUP дозволяє.

**4. Рівень escrow-арбітражу.**
Сегрегований trust-рахунок через ліцензованого еквайра/MoR (уникнути money-transmitter статусу) чи власна ліцензія платіжної установи НБУ?
- **Рекомендація: trust-рахунок через ліцензованого партнера** на старті (швидше, дешевше); власна ліцензія — лише при масштабі. Юрвисновок обов'язковий ПЕРЕД live.

**5. Cash-flow організаторів.**
Early-payout (+3%) і/або частковий аванс перевіреним? Який rolling reserve?
- **Рекомендація:** так, early-payout +3% з M5; аванс до 50% лише перевіреним з низьким chargeback. Rolling reserve ~5-10% на 90 днів для нових організаторів.

**6. Рівень комісії.**
Фіксувати 7-10% all-in для всіх чи диференціювати? Free точно 0%?
- **Рекомендація:** 8% all-in база, нижче для Pro-підписки/перевірених. **Free події — точно 0%** (cold-start).

**7. Anti-scalping vs gifting.**
Наскільки жорстко прив'язувати квиток до особи (DICE-контур)?
- **Рекомендація:** іменні квитки + transfer/resale тільки через систему (face-value), але gifting лишити зручним (1 transfer на квиток через email). Баланс: контур закритий, подарунки можливі в межах системи.

**8. KYC-провайдер.**
Sumsub/Onfido (глобальний) чи Diia.Підпис/Diia.Card (UA-нативний, дешевший, лише UA-резиденти)?
- **Рекомендація: Diia як основний для UA-організаторів** (дешевше, нативно), Sumsub/Onfido як fallback для не-резидентів. Поріг KYC: усі організатори, що отримують payout (KYB-gate на `PAID_OUT`).

**9. Resale-політика.**
Face-value P2P-resale у v1 чи відкласти? Стеля = номінал чи номінал + комісія?
- **Рекомендація:** відкласти на M5 (після ліквідності); стеля = строго номінал (комісія платформи зверху, не markup продавця).

**10. Geo-scope.**
UA/UAH/Київ чи рух до worldwide?
- **Рекомендація:** Київ-first, UA/UAH. Worldwide передчасно (i18n/FX/legal EAA/DSA). Density важливіша за охоплення.

**11. Воєнне правило рефандів.**
Інтегрувати air-alert API як сигнал авто-вирішення чи лишити ручним?
- **Рекомендація:** інтегрувати air-alert фід як **сигнал-підказку** для admin-dispute (не повністю авто) на M3; юридична валідність «70-80% = відбулась» — підтвердити з counsel і прописати в Terms.

**12. Послідовність запуску.**
Closed beta (обмежене коло організаторів) чи відкритий self-serve одразу?
- **Рекомендація: closed beta на Києві** з 20-50 промоутерами/клубами (fee-holiday) — безпечніше для P0-ризиків (гроші/compliance), дає density перед «network flip».