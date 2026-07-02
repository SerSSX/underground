# Underground v2 — Зміни продукту (ДОДАТИ / ПРИБРАТИ / ПОКРАЩИТИ) + payment-agnostic архітектура

> Документ #1. Governing doctrine: `docs/rules.md` (v0.5.0, STRICT). Усі грошові потоки — через єдиний seam. Тікетинг та escrow — НАШ домен, provider-neutral.
> Статус коду — за реальними файлами `api/src/*.rs`, `android/app/src/main/java/com/underground/app/ui/**`, `web/app/**`, `admin/app/**`.

---

## 1. Принцип №1: PRODUCT-FIRST + payment-as-a-plugin

**Теза:** ми будуємо повноцінний, придатний до запуску і тестування продукт **зараз**, ще до того, як підключимо реальну платіжну систему. Платіж — це **плагін**, а не фундамент.

### Як продукт працює БЕЗ реальної платіжки

| Механізм | Що це дає | Як реалізовано (provider-neutral) |
|---|---|---|
| **Безкоштовні події / RSVP** | Користувач отримує квиток без жодної оплати (`reserve()` → mint). Запуск і органічне зростання можливі вже сьогодні. | `cost_type='free'` або `tier.price=0` → шлях `claim_free_ticket` повз `order/provider`. Жоден провайдер не викликається. |
| **`NoopProvider`** | Безкоштовний шлях формально проходить через seam (єдиний код-шлях, без розгалужень «free vs paid» у домені). | `create_charge()` миттєво повертає `status=Paid` без зовнішніх викликів. |
| **`TestProvider`** | Повний цикл платної покупки тестується локально/в CI: checkout → «оплата» → e-ticket → escrow HELD → CLEARED → payout. | `pay-test` (нинішній `orders.rs`) переноситься за seam. Детерміновані `provider_ref`. Admin «mark-as-paid» / «mark-as-settled». |
| **Admin mark-as-paid / mark-as-settled** | Оператор вручну просуває order/payout у CI чи демо без реальних грошей. | `POST /api/admin/orders/:id/mark-paid` → `orders::fulfill`; під `TestProvider` settle миттєвий. |

**Висновок:** на старті активний провайдер = `Noop`/`Test`. Продукт повністю функціональний — discovery, квитки, gifting, refund, escrow lifecycle, payout (тестовий), social, admin — усе працює і покривається тестами **до** інтеграції з реальним PSP.

### Як будь-яка платіжка підключається пізніше

Реальний провайдер (Stripe Connect, Mangopay, Rapyd, Payoneer, LiqPay, Lemon Squeezy) реалізує **лише 5 дій** поверх нашого незмінного escrow-домену:

```
charge → hold → transfer(payout) → refund → verify_webhook
```

Жоден рядок ticketing/escrow-логіки не змінюється. Перемикання — через `payment_settings.provider` в адмінці. Це і є директива №2.

---

## 2. Архітектура payment-agnostic

### 2.1. trait `PaymentProvider` (єдиний дозволений дотик до грошей)

Новий модуль: `api/src/payments/provider/mod.rs`. Тікетинг (`tickets.rs`, `orders.rs`) та `escrow.rs` викликають **тільки** цей trait — ніколи не гілкуються на `provider == "lemonsqueezy"`.

```rust
#[async_trait]
pub trait PaymentProvider: Send + Sync {
    async fn create_charge(&self, req: ChargeReq)   -> Result<ChargeResult>;
    async fn capture(&self, provider_ref: &str)      -> Result<()>;
    async fn hold(&self, provider_ref: &str)         -> Result<()>;   // escrow-hold (або симулюється в домені)
    async fn transfer_payout(&self, req: PayoutReq)  -> Result<TransferResult>;
    async fn refund(&self, req: RefundReq)           -> Result<RefundResult>;
    fn verify_webhook(&self, headers: &HeaderMap, raw: &[u8]) -> Result<WebhookEvent>;
}
```

### 2.2. DTO (provider-neutral, без жодного LS-специфічного поля)

| DTO | Поля | Призначення |
|---|---|---|
| `ChargeReq` | `order_id`, `amount_minor: i64`, `currency`, `buyer_ref`, `return_url` | Запит на списання |
| `ChargeResult` | `provider_ref`, `checkout_url: Option<String>`, `status: ChargeStatus` | Результат: hosted-redirect, SDK-sheet або миттєвий success |
| `PayoutReq` | `payout_id`, `organizer_ref`, `amount_minor`, `currency` | Виплата організатору |
| `TransferResult` | `provider_ref`, `status: PayoutStatus` | requested/processing/paid/failed |
| `RefundReq` | `provider_ref`, `amount_minor`, `reason` | Повернення |
| `RefundResult` | `provider_ref`, `status` | Підтвердження |
| `WebhookEvent` | `kind: Paid\|Refunded\|PayoutSettled\|PayoutFailed\|ChargeFailed`, `order_ref/payout_ref`, `provider_ref` | Нормалізована подія від будь-якого PSP |

**Реалізації:** `NoopProvider` (free/RSVP, dev) · `TestProvider` (admin mark-paid/settled, детерміновано) · `LemonSqueezyProvider` (увесь нинішній `ls_*`-код переїздить сюди). Registry резолвить провайдера за `payment_settings.provider`.

### 2.3. `escrow.rs` лишається provider-neutral

- `escrow.rs` — це **state machine** `HELD→CLEARING→CLEARED→PAID_OUT` (+ гілки `DISPUTED`/`REFUNDED`), а НЕ абстракція провайдера.
- Background `sweep` просуває кошти за `ends_at + clearing_window_days`, **не чіпаючи** `frozen/disputed`.
- Провайдер лише **виконує** charge/hold/transfer/refund поверх цих станів. Escrow ніколи не знає, ХТО провайдер.
- Admin-дії refund/release/payout тепер **реально** викликають seam (сьогодні це чисті DB-flip, що не рухають грошей) — записують `provider_ref` і статус.

### 2.4. Зміни схеми БД

| Міграція | Таблиця/поле | Призначення |
|---|---|---|
| `00xx_payment_provider.sql` | `payment_settings`: generic `provider`, `provider_config JSONB` (замість захардкоджених `ls_*`) | Provider-agnostic конфіг |
| | `orders`: `provider TEXT`, `provider_ref TEXT`, `dispute_reason TEXT`, `disputed_at TIMESTAMPTZ` | Трасування PSP + контекст диспуту |
| | `payouts`: `provider_ref TEXT`, статус-enum `pending\|processing\|paid\|failed` | Реальний transfer + очікування webhook |
| `00xx_ledger.sql` | `ledger_entries(entry_type, order_id/payout_id, amount_minor, currency, direction debit\|credit, provider_ref, created_at)` | Append-only double-entry для звірки |
| `00xx_ticket_tiers.sql` | `ticket_tiers(id, event_id, name, price, currency, capacity, sold, sort_order, sales_start_at, sales_end_at, min_per_order, max_per_order, visibility, active)` | Тарифи; міграція `events.price/tickets_total` → дефолтний tier «Standard» |
| `0033_event_staff.sql` | `event_staff(id, event_id, user_id NULL, invited_email, perms TEXT[], status, invite_token UNIQUE, invited_by, created_at, accepted_at)` | Делегування door-staff |
| `00xx_scan_audit.sql` | `tickets.scanned_by UUID`; `scan_events(ticket_id, event_id, scanner_id, result, scanned_at)` | Аудит сканів |

### 2.5. Кроки міграції з Lemon Squeezy без поломки CI

1. **Ввести trait + `NoopProvider`/`TestProvider`** (не чіпаючи LS-код). CI зелений — нові юніт-тести seam.
2. **`LemonSqueezyProvider`**: перенести `format!("https://{store}.lemonsqueezy.com/buy/…")` з `orders.rs:127-135` та HMAC з `webhooks.rs` за trait. Поведінка ідентична — інтеграційні тести LS лишаються.
3. **Generic webhook**: `POST /api/webhooks/:provider` → `provider.verify_webhook`. Старий LS-маршрут — alias на період міграції.
4. **`orders.rs`/`payouts.rs`** викликають `provider.*` замість inline. Видалити LS-гілки з домену.
5. **Admin-дії** (refund/release/payout) проводимо через seam.
6. Прибрати alias-маршрут після підтвердження. Кожен крок — окремий PR, CI зелений (§8.6 `./scripts/ci.sh`).

### 2.6. Потік грошей через seam (діаграма-список)

```
ПОКУПКА (paid)
  Attendee → checkout {tier_id, qty, promo}
    orders::checkout → provider.create_charge()
      ├─ Noop/Test: status=Paid одразу
      ├─ hosted PSP: checkout_url → redirect → webhook Paid
      └─ SDK PSP: client_action=native_sheet → webhook Paid
    orders::fulfill → mint N tickets, escrow_status=HELD, ledger: credit HELD
                                          │
ESCROW SWEEP (фон, provider-neutral)      ▼
    ends_at + clearing_window → HELD→CLEARING→CLEARED   (frozen/disputed пропускаються)
                                          │
ВИПЛАТА                                   ▼
    organizer payout-request / admin batch → provider.transfer_payout()
      → status=processing → webhook PayoutSettled → PAID_OUT, ledger: debit
                                          │
ПОВЕРНЕННЯ                                ▼
    return/refund/dispute → provider.refund() → escrow REFUNDED, seat freed,
                            promo restored, ticket returned, ledger: reverse
```

---

## 3. ГОЛОВНА ТАБЛИЦЯ ЗМІН

Консолідовано всі surfaces (Android / Web / Admin / API), без дублів. `Тип`: ➕ ДОДАТИ · ➖ ПРИБРАТИ · 🔧 ПОКРАЩИТИ · ✅ ЗБЕРЕГТИ.

### 3.1. Discovery (Афіша / пошук / стрічка подій)

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| ➕ | **Параметризований пошук**: `GET /api/events?q=&city=&category=&date_from=&date_to=&price_max=&free=&sort=&cursor=` з курсор-пагінацією. Android `AfishaScreen` + Web `/home` SSR. | **P0** | Зараз лише ForYou/All + клієнтська фільтрація однієї сторінки (`AfishaScreen.kt:79-82`, `web/app/page.tsx`) — discovery неконкурентний. |
| ➕ | **Search bar** + сегменти `Для тебе \| Усі \| Друзі йдуть \| Безкоштовні` + рядок фільтрів `Місто/Дата(Сьогодні/Завтра/Вихідні/Діапазон)/Категорія/Ціна/Сортування(Дата/Популярність/Поблизу)`. | **P0** | Без пошуку й дат користувач не знаходить подію. |
| ➕ | **Friends-going соц-доказ у `EventCard`**: рядок аватарок «N друзів іде» з discover-відповіді. | P1 | Соцмережа — диференціатор; зараз соц-доказ лише на Деталях текстом. |
| ➕ | **Pull-to-refresh + нескінченний скрол** (`?cursor=`), pull-to-refresh, секції «Рекомендовано/Біля тебе/Друзі йдуть». | P1 | Утримання й масштаб стрічки. |
| ➕ | **Web SEO**: SSR `/events`, лендінги `/c/[category]`, `/city/[slug]`, JSON-LD `Event`/`ItemList`, sitemap із подіями. | P1 | Органічний трафік — головна перевага web; зараз `page.tsx` = 100% `'use client'`. |
| 🔧 | **Серверна фільтрація на web `/home`**: слати `city/free/sort` у `getEvents` (API підтримує, UI не слав), map-view за `lat/lng`. | P1 | `page.tsx` фільтрує в пам'яті — зламано на масштабі. |

### 3.2. Event & Tickets (модель події, тарифи, видимість)

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| ➕ | **Ticket tiers**: таблиця `ticket_tiers(...)`; `EventDto.tiers: List<TierDto>`; `CreateEventReq/UpdateEventReq.tiers: Vec<TierInput>`; endpoints `POST /api/events/:id/tiers`, `PUT /api/tiers/:id`, `DELETE /api/tiers/:id` (owner-only). Мінт і decrement — per-tier (`ticket_tiers.sold/capacity`). | **P0** | Зараз один `event.price` + один лічильник (`events.rs:62-84`); GA/VIP/Early-bird неможливі. |
| ➕ | **Quantity у checkout**: stepper `1..max(left, max_per_order)`; мінт N квитків в одному order. | **P0** | Зараз 1 квиток за раз — неможливо купити для компанії. |
| ➕ | **Visibility `unlisted/private`**: розширити `status` (draft/published/**unlisted**/hidden/cancelled); `list_events` показує лише published. | P1 | Корпоративи/приватні події (`events.rs:228,314`). |
| ➕ | **(резерв) `event_sessions(event_id, starts_at, ends_at, capacity)`** для багатоденних/повторюваних. | P2 | Фестивалі; модель зарезервувати в MVP. |
| 🔧 | **Create-event форма** (Web `EventForm.tsx`, Android `CreateEventScreen`): динамічний редактор tiers (name/price/capacity/sales window/max_per_order), `ends_at` picker, timezone, валідація Σcapacity; **draft-перемикач в Android** (зараз лише web). | **P0** | Зараз одне поле Ціна+Кількість; Android без чернетки. |
| ✅ | **Free-RSVP / `reserve()` без оплати** — зберегти. | **P0** | Директива №1: продукт працює без провайдера. |

### 3.3. Checkout & Payments (provider-agnostic seam)

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| ➕ | **`trait PaymentProvider`** (charge/hold/transfer_payout/refund/verify_webhook) + impl `Noop/Test/LemonSqueezy`; registry за `payment_settings.provider`. | **P0** | Директива №2; зараз LS захардкоджено в `orders.rs`/`webhooks.rs`/`payments.rs`. |
| ➕ | **`checkout()` повертає `provider_action {kind: test_autopay\|redirect\|none\|native_sheet, url?}`**; клієнти діють за ним. | **P0** | Web `EventDetailClient.confirmBuy` завжди викликає `payTest` (рядки 126-140) — прибито до тесту. |
| ➕ | **Поле промокоду в checkout UI** (Android + Web): `TextField` + «Застосувати» → `checkout(?promo=)`, показ знижки. | P1 | `apply_promo` є в API, але UI промо не шле — фіча мертва. |
| ➕ | **Зведення у checkout**: line-items (tier×qty), subtotal, сервісний збір, **ВСЬОГО** (`fmtPrice` з currency), чекбокс згоди з ескроу/refund-політикою, receipt-сторінка. | P1 | Прозорість покупки; зараз немає розбивки. |
| ➕ | **Generic webhook** `POST /api/webhooks/:provider` → `verify_webhook` просуває order/payout. | **P0** | Сьогодні лише LS-webhook. |
| ➕ | **Guest checkout** (web): `reserve/checkout` з `guest {email, name}`; magic-link для claim; e-ticket поштою. Free завжди; paid — якщо провайдер дозволяє. | P1 | Shareable-лінки конвертують без signup-wall; зараз buy форсить `/login`. |
| 🔧 | **Стани оплати** (processing/failed/success) — уніфікований результат, ретрай; не залежить від провайдера. | P1 | TestPay має лише busy/error; LS — зовнішній URL без повернення стану. |
| ➖ | **Хардкод LS URL** з `orders.rs:127-135` → `provider.create_charge()`. | **P0** | Порушує §10 (хардкод redirect) і provider-agnostic ціль. |
| 🔧 | **Admin escrow refund/release + payout** реально викликають `provider.refund()/transfer_payout()` (зараз чисті DB-flip). | **P0** | Без цього release/refund/payout не рухають грошей. |
| ✅ | **Escrow lifecycle** `HELD→CLEARING→CLEARED→PAID_OUT` + dispute + sweep. | **P0** | Наш домен, конкурентна перевага; оточити trait-ом, не переписувати. |

### 3.4. Gifting & Refund

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| ➕ | **Gift-at-checkout**: `checkout {gift: {recipient_email, message}}` → мінт квитка одразу на отримувача. | P1 | Зараз gifting лише пост-покупка (`transferTicket`). |
| 🔧 | **Transfer не лише по email**: діалог `email АБО @username/контакт` → `POST /api/tickets/:id/transfer`; кнопка дарунка на картці у «Мої квитки», не лише в e-ticket. | P2 | Низька помітність (`ETicketScreen.kt:167`). |
| 🔧 | **Refund UX з політикою/сумою**: показати суму повернення, строки (escrow `held`→повне; після `clearing` — за політикою), причину, чітке підтвердження. | P1 | Зараз «Повернути» без контексту; ескроу незрозумілий. |
| 🔧 | **Поділ «Мої квитки»**: сегменти `Майбутні \| Минулі \| Повернені/Подаровані`, міні-постер (`eventPosterUrl` уже в `TicketDto`), мітка «подарунок». | P1 | Плоский список без поділу (`MyTicketsScreen.kt`). |

### 3.5. Door-staff / Delegation & Ticket Validation

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| ➕ | **`event_staff` + helper `staff::require_perm(db, event_id, uid, perm)`** (`Owner\|Staff(perms)\|None`); perms `scan/view_list/refund/manage_staff`. STAFF — **per-event relationship**, НЕ глобальна роль. | **P0** | Зараз сканує лише власник (`tickets.rs:179/181`); делегування неможливе без передачі акаунта. |
| ➕ | **Staff endpoints**: `POST/GET /api/events/:id/staff`, `DELETE …/staff/:uid`, `POST /api/staff/invites/:token/accept`, `GET /api/me/staff-events`. | **P0** | Запрошення/прийняття/перелік відсутні в `routes.rs`. |
| ➕ | **Scan audit**: `tickets.scanned_by`, `scan_events(...)`; `validate()` пише `scanned_by` + scan-event. | P1 | Хто впустив квиток — для фрод/диспутів (зараз лише `validated_at`). |
| ➕ | **Offline-черга на Android** (Room): при втраті мережі — локальна перевірка по кешу + replay; сервер ідемпотентний за `(qr_token, scanned_at)`. | P1 | Підвали без зв'язку; зараз HTTP на кожен скан (`ScanTickets.kt:66-68`). |
| 🔧 | **`POST /api/tickets/validate` стає event-scoped**: приймає `{qr_token, event_id}`; вердикти `ok\|already_used\|wrong_event\|refunded\|returned\|inactive`; повертає `owner_name, tier, validated_at, scanned_by_name`; ідемпотентно `FOR UPDATE`. | **P0** | Зараз ігнорує подію; набір вердиктів бідний (`tickets.rs:183-193`). |
| 🔧 | **`GET /api/events/:id/tickets`** через `require_perm(view_list)`; `?status=` + пагінація; приховати email для staff без `pii`. | P1 | Контролерам потрібен список для ручної звірки (`organizer.rs:123`). |
| 🔧 | **Android Door Mode (event-scoped)**: header `просканнано N/M`, безперервний скан, повноекранний кольоровий вердикт (зелений/червоний/бурштин/сірий), beep+haptic, лог останніх 10, пошук за іменем. | **P0** | Зараз одноразовий event-agnostic сканер (`ScanTickets.kt:44`). |
| ➕ | **Staff home «Мої зміни»** (Android tab + web `/staff`): `GET /api/me/staff-events`, accept deep-link `/staff/invites/:token`. | P1 | Делегат не власник → `MyEventsScreen` йому нічого не показує. |
| ➕ | **`refund` perm path**: `POST /api/events/:id/tickets/:tid/staff-return` (через escrow seam), за замовчуванням НЕ в інвайті. | P2 | On-site refunds; чутливо — opt-in. |
| ✅ | **Owner-implicit full access** + escrow як єдиний грошовий seam (staff-return проходить через нього). | **P0** | Зберегти ownership-модель та escrow-інваріант. |

### 3.6. Organizer cabinet (онбординг, керування, аналітика, виплати)

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| ➕ | **Become-organizer онбординг** (web `/organizer/onboarding` + Android): крок 1 профіль (`display_name, bio, avatar, payout_country`) `POST /api/me/organizer-profile`; крок 2 KYC `POST /api/me/kyc`; крок 3 «створіть першу подію». | P1 | Зараз «стати організатором» = форма KYC всередині `account/page.tsx:96-120`; Android KYC взагалі нема. |
| ➕ | **Organizer payout request**: `POST /api/organizer/payouts/request` (валідація: kyc verified + cleared>0); кнопка «Запросити виплату». | P1 | Зараз payout створює лише admin (`payouts.rs:31`); організатор read-only. |
| ➕ | **Check-in / live stats**: `GET /api/events/:id/checkin-stats` → `{capacity, sold by tier, checked_in, rate, last_scans[]}`; екран «Контроль на вході». | P1 | Зараз лише сумарний `tickets_validated` (`organizer.rs:34-36`). |
| 🔧 | **Attendee list**: колонки `tier_name, promo_code`, фільтри статус/tier, пошук, **«надіслати квиток повторно»** `POST /api/tickets/:id/resend`, експорт із tier/promo. | P1 | Зараз без tier/promo/фільтрів/resend (`organizer.rs:101-109`). |
| 🔧 | **Analytics**: вибір періоду (7/30/усе/кастом), воронка `перегляди→checkout→оплачено`, revenue-by-tier, refund/return rate, sell-through %, top promos. | P1 | Зараз фікс 30 днів + проста гістограма (`organizer.rs:60-74`). |
| 🔧 | **Payouts view**: per-event breakdown held/clearing/cleared + очікувана дата clearing (`ends_at + clearing_window_days`); provider-нейтральний копірайт. | P2 | Зараз лише агреговані числа без розбивки. |
| 🔧 | **Promo editor**: `max_uses`, `expires_at`, прив'язка до tier, fixed-amount поряд із percent. | P2 | API приймає (`promos.rs:42-47`), UI ні. |
| ✅ | **`require_owner` guards + KYC payout gate** (`blocked=false AND kyc_status=verified`). | **P0** | Делегування — адитивне поверх, не послаблює owner-only. |

### 3.7. Social Network (профілі, follow, стрічка, пости, відгуки, DM, сповіщення)

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| 🔧 | **Стрічка: ранжування + пагінація + таби**: `GET /api/feed?cursor=<created_at,id>&filter=for_you\|following\|popular`; `score = recency*w1 + (likes+comments)*w2 + friends_boost`; Android chip-таби + pull-to-refresh + infinite scroll. | **P0** | Плоский `created_at` LIMIT 50 без рефрешу (`posts.rs:148-170`) не масштабується. |
| ➕ | **RSVP/save та follow як feed-об'єкти**: «N друзів іде на <Подія>» (аватар-стек), «X підписався на <Організатор>»; `POST /api/events/:id/rsvp {state: going\|interested\|none}` живить `EventSocialDto`. | **P0** | «Друзі йдуть» — найсильніший драйвер віральності; на Деталях немає RSVP-кнопок попри показ лічильників. |
| ➕ | **Verified-attendee gate відгуку**: `reviews.rs` — `EXISTS ticket WHERE owner_id=user AND event_id AND status IN('active','used')` ПЛЮС `starts_at<now`; поле `verified_attendee` + бейдж «Був(ла) на події». | **P0** | Зараз гейт лише за часом — будь-хто оцінює без квитка (`reviews.rs:56-65`), підрив довіри. |
| ➕ | **Block / Mute**: `user_blocks(blocker_id, blocked_id)`; `POST/DELETE /api/users/:id/block,/mute`; фільтрація у feed/comments/posts/messages/social-proof. | **P0** | App Store/Play вимога; зараз лише report (`reports.rs:15`). |
| ➕ | **Фото/медіа** у постах/відгуках/DM: `posts.media_urls text[]`, `reviews.media_urls text[]`, `messages.attachment_url/type` через `uploads.rs`. | P1 | Текст-онлі контент мертвий; фото з подій — органічний контент. |
| ➕ | **Екрани Followers/Following + клікабельні лічильники** (Android/web) поверх готових `GET /api/users/:id/followers\|following`; inline follow/«follow back». | P1 | API готове, UI нема — соц-граф невидимий (`social.rs:139-163`). |
| ➕ | **Invite / Share**: `POST /api/events/:id/invite {user_ids[]}` (notification+DM); deep-link `underground://event/:id`, `/user/:id`; repost поста. | P1 | Головний low-cost virality loop; зараз відсутній. |
| ➕ | **Композер поста** (окремий екран): `text(≤2000), media_urls[0..4], event_id, visibility(public\|followers)`; FAB у стрічці. | P1 | Зараз inline тільки текст. |
| ➕ | **Лайки/відповіді/видалення коментарів**: `comment_likes`, `post_comments.parent_id`; `POST/DELETE /api/comments/:id/like`, `DELETE /api/comments/:id`. | P2 | Неповноцінне обговорення знижує engagement. |
| ➕ | **`@handle` + редагування профілю + QR-візитка**: `users.handle UNIQUE`; екран edit (ім'я/bio/avatar/handle). | P2 | Для згадок, шерингу, офлайн-обміну на подіях. |
| 🔧 | **Аватар-фото + verified-бейдж** у `PublicProfileScreen` (рендерити `avatar_url`+`verified`), `PostCard`/`CommentCard`/`Conversation` (`AsyncImage`). | P1 | Дані вже в API, UI ігнорує (`PublicProfileScreen.kt:87-97`) — мережа безлика. |
| 🔧 | **Соц-сповіщення → правильний deep-link + групування + аватари**: `AlertsScreen` використовує `actor_id` (→ профіль/пост), агрегація «X та ще N…», фільтр `Соціальні\|Події`. | P1 | Зараз тап веде лише на подію (`AlertsScreen.kt:102`); `actor_id` не використовується. |
| 🔧 | **DM real-time**: WebSocket `/ws` (auth-JWT) + typing + presence; read-receipt галочки (поле `read` є). REST як фолбек. | P2 | Зараз 4с polling — затримка+батарея (`ChatScreen.kt:95-100`). |
| ➕ | **Follow-організатора з пуш-опцією + відповідь організатора на відгук**: `follows.notify`; `POST /api/reviews/:id/reply`, `POST /api/reviews/:id/helpful`. | P2 | Канал ретеншну + репутаційний інструмент маркетплейсу. |
| ✅ | **Ескроу-нейтральність соц-шару**: жоден соц-endpoint не чіпає грошей; review/RSVP лише читають `tickets.status`. | **P0** | Директива: соц-логіка не викликає `escrow.rs`/`payments`. |

### 3.8. Auth & Onboarding & Profile (attendee)

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| 🔧 | **Google-only auth по spec** (Android+Web): робочий Google Sign-In `id_token → POST /api/auth/google → app-JWT`; згода з Умовами; прибрати email/password (де spec вимагає). | **P0** | Doctrine «тільки Google OAuth»; зараз Google — Toast-заглушка (`AuthScreen.kt:114`), а працює email/password — пряме порушення. |
| ➕ | **Onboarding 3 кроки**: (1) категорії + місто + «Поблизу мене» (геолокація, радіус); (2) **DOB** `PATCH /api/me {dob}`; (3) follow-suggestions `POST /api/follow/:id` + push opt-in `POST /api/push/register`. | P1 | Зараз лише категорії+місто; DOB схований у Settings → 18+ checkout мовчки падає 409. |
| 🔧 | **Age-gate UX**: при 18+ без DOB — інлайн-підказка з переходом, не глухий 409. | P1 | `orders.rs meets_age` падає мовчки. |
| ➕ | **Push opt-in + налаштування сповіщень** у Settings (категорії: нагадування про подію, друзі йдуть, відповіді організатора). | P1 | `push.rs` є, attendee-surface без opt-in — страждає ретеншн. |
| 🔧 | **Profile editing на web** (`patchMe`: name/bio/avatar), notification prefs, мова, link Google, список збережених. | P1 | UI не використовує наявний `patchMe` (`account/page.tsx`). |

### 3.9. E-Ticket & QR Security

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| 🔧 | **Rotating/short-lived QR** (HOTP/TOTP або JWT з `exp`); `keepScreenOn` + макс. яскравість при показі. | **P0** | Статичний токен — тривіальна підробка/двічі-вхід; SEC-ризик контролю. |
| ➖ | **Сирий `qrToken` текстом під QR** (`ETicketScreen.kt:98` `Text(ticket.qrToken)`). | **P0** | Витік секрету сканування; користувачу не потрібен. |
| ➕ | **Full-ticket / public door page (web)**: `/me/tickets/[id]` (owner) + signed `/t/[token]` (guest); full-screen QR, Apple/Google Wallet, print stylesheet (ICS уже є); `getTicket(id)`. | P1 | Web-парність; зараз лише inline QR у гріді. |
| 🔧 | **Прибрати хардкод `₴`** у `web/.../me/tickets:163` → `fmtPrice(t.price, currency)`; додати `currency` у `TicketDto`. | P2 | Going worldwide. |

### 3.10. Platform Admin / Operator back-office

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| ➕ | **Dashboard financial KPIs**: `GET /api/admin/stats` += `escrow_held_total, escrow_cleared_pending_payout, paid_out_total, refunded_total, disputed_count, pending_payouts_amount, kyc_pending_count, gmv_7d/30d` (per-currency) + `GET /api/admin/stats/timeseries?metric=gmv\|fees\|orders&days=`. | **P0** | Оператору потрібна видимість liability (held funds) і тренду; зараз лише users/events/tickets/GMV. |
| ➕ | **Order/dispute detail drawer**: `GET /api/admin/orders/:id` (buyer, organizer, event, provider, provider_ref, escrow_status, ticket, promo, dispute_reason+disputed_at, timeline); `POST /api/orders/:id/dispute {reason}` зберігає reason. | **P0** | Зараз таблиця без контексту чому order frozen. |
| ➕ | **Finance / reconciliation**: `/finance` + `GET /api/admin/reconciliation?from&to&currency` + `GET /api/admin/export/{orders\|payouts\|refunds\|ledger}.csv`; append-only `ledger_entries`. | **P0** | MoR/escrow вимагає звірки held/cleared/paid-out + експорт; зараз нема. |
| ➕ | **KYC/KYB queue** `/kyc`: `GET /api/admin/kyc?status=&kind=`; review drawer (payout_name, IBAN masked, SWIFT, business name/reg/doc_url), approve/reject **з reason** (audited); `kyc_kind individual\|business`. | P1 | Зараз KYC — inline-кнопки в Users без контексту/reason. |
| ➕ | **Anti-fraud signals** `/risk`: `GET /api/admin/risk/signals` (refund rate, dispute rate, velocity, IBAN reuse, new-organizer high-value); `risk_score/risk_flags` на AdminUser. | P1 | Escrow+payouts роблять фрод ключовим ризиком; зараз нема. |
| ➕ | **Bulk payout run**: `POST /api/admin/payouts/run` (усі організатори з cleared+verified, batch); `GET /api/admin/payouts/:id` (включені orders); статуси `pending\|processing\|paid\|failed`. | P1 | Зараз треба вручну вставляти UUID по одному. |
| 🔧 | **Users filters + detail**: `?role=&blocked=&kyc_status=&verified=`; `GET /api/admin/users/:id` (events, GMV, refund/dispute rate, escrow held, payouts, risk). | P1 | Зараз плоский список з text-search; модерація наосліп. |
| 🔧 | **Reports queue**: target preview snapshot (event title/post excerpt/review text/email) замість обрізаного UUID; enforcement warn/hide/restrict-payouts; групування за target. | P1 | Реєвери бачать лише `target_id.slice(0,8)…`. |
| 🔧 | **Payment settings provider-agnostic**: селектор `{noop, test, lemonsqueezy, stripe…}`; динамічні поля з `GET /api/admin/payment-providers`; `platform_fee_bps`+`clearing_window_days` як provider-neutral; readiness checklist. | P1 | Зараз захардкоджені `ls_store_id/ls_variant_id/ls_api_key`. |
| 🔧 | **Audit log filters + export**: `GET /api/admin/actions?admin_id=&action=&target_type=&from=&to=` + CSV; читабельний detail; deep-link target. | P2 | Зараз 200 рядків, клієнтський фільтр. |
| 🔧 | **Dashboard alert strip → live counts**: open reports, disputed/frozen, pending payouts, KYC pending — кожен deep-link на фільтр. | P2 | Зараз статичні лейбли без лічильників. |
| ✅ | **Append-only audit у тій самій транзакції** + **KYC-gated payout** + **escrow sweep лишає frozen**. | **P0** | Compliance-grade; нові provider-дії мають це успадкувати. |

### 3.11. Platform / Infra

| Тип | Що саме (функції / поля / формат) | Пріор. | Навіщо |
|---|---|---|---|
| ➕ | **`ledger_entries`** (double-entry) пишеться при кожній зміні грошового стану. | **P0** | Основа звірки/податків. |
| ➕ | **Generic webhook receiver** `POST /api/webhooks/:provider`. | **P0** | Замість LS-специфічного. |
| ➕ | **Локалізовані нотифікації (uk/en)** через `notify_actor` — поширити патерн на `invite/comment_like/review_reply`. | P1 | Готова i18n для worldwide. |
| 🔧 | **Operator MFA/TOTP** для privileged admin-акаунтів. | P2 | Захист back-office; зараз лише email/password. |

---

## 4. Executive summary — ДОДАЄМО / ПРИБИРАЄМО / ПОКРАЩУЄМО

### ➕ ЩО ДОДАЄМО

| Область | Ключове | Пріор. |
|---|---|---|
| Payments | `trait PaymentProvider` (Noop/Test/LS) + `provider_action` + generic webhook + ledger | **P0** |
| Tickets | Ticket tiers + quantity + visibility unlisted/private | **P0** |
| Door-staff | `event_staff` + `require_perm` + staff endpoints + Door Mode + staff home | **P0** |
| Social | RSVP/save як feed-об'єкти + verified-attendee gate + block/mute + cursor-feed | **P0** |
| Discovery | Параметризований пошук + фільтри + SEO-лендінги | **P0** |
| Admin | Financial KPIs + order/dispute drawer + finance/reconciliation + KYC/KYB queue + risk + bulk payouts | **P0–P1** |
| Checkout | Промокод UI + зведення + guest checkout + gift-at-checkout | P1 |
| Social+ | Медіа в постах + Followers/Following екрани + invite/share + композер | P1 |
| Onboarding | DOB-крок + push opt-in + follow-suggestions | P1 |

### ➖ ЩО ПРИБИРАЄМО

| Що | Де | Пріор. |
|---|---|---|
| Хардкод Lemon Squeezy URL | `orders.rs:127-135` → за trait | **P0** |
| Сирий `qrToken` текстом під QR | `ETicketScreen.kt:98` | **P0** |
| Email/password auth (де spec вимагає Google-only) | `AuthScreen.kt`, web login | **P0** |
| Frontend coupling `checkout→payTest` | `EventDetailClient.confirmBuy:126-140` | **P0** |
| Захардкоджені `ls_*` поля в payment settings UI | admin `/payments` | P1 |

### 🔧 ЩО ПОКРАЩУЄМО

| Що | Ефект | Пріор. |
|---|---|---|
| `validate()` → event-scoped + багаті вердикти + audit | Безпечний контроль входу з делегуванням | **P0** |
| Feed → ранжування + пагінація + таби | Retention | **P0** |
| Create-event форма → tiers editor | Будь-який масштаб подій | **P0** |
| Admin escrow refund/payout → реально через seam | Реальна back-office | **P0** |
| Google Sign-In робочий | Відповідність spec | **P0** |
| Серверна фільтрація discovery (web) | Масштаб | P1 |
| Refund UX з сумою/політикою | Trust | P1 |
| Аватари/verified у профілях і постах | Довіра до мережі | P1 |
| Соц-сповіщення → deep-link+групування | Повернення в апп | P1 |
| QR rotating + keep-screen-on | SEC контролю входу | **P0** |

---

## 5. Чому це зробить продукт популярним

| Зв'язок | Зміни-драйвери | Метрика, що зростає |
|---|---|---|
| **Trust (довіра)** | Escrow-захист покупця (гроші утримуються до події), verified-attendee відгуки, rotating QR, refund з прозорою сумою, KYC-gated payout, dispute-flow. | Conversion (готовність купити у незнайомого організатора), низький chargeback. |
| **Virality (віральність)** | «Друзі йдуть» у картці+стрічці, RSVP як соц-об'єкт, invite/share deep-link, gift-at-checkout, guest checkout без signup-wall, SEO-лендінги. | K-factor, частка трафіку з шерингу/органіки. |
| **Retention (утримання)** | Cursor-feed з ранжуванням і табами, push-нагадування «друзі йдуть/нова подія організатора», follow + сповіщення з deep-link, медіа в постах. | DAU/MAU, повернення D7/D30. |
| **Frictionless purchase** | Tiers + quantity + промокод + Apple/Google Pay (через seam) + зрозуміле зведення. | Checkout completion rate, середній чек (group buys). |
| **Organizer success** | Tiers, аналітика-воронка, door-staff делегування без передачі акаунта, live check-in, payout-request. | Кількість активних організаторів, повторні події. |
| **Worldwide-readiness** | Provider-agnostic seam (LiqPay для UA, Stripe/Mangopay для світу), мульти-валюта, i18n нотифікації, KYB. | Вихід на нові ринки без переписування. |

Головне: **продукт можна запустити й полюбити вже на безкоштовних подіях + Test-провайдері**, накопичити аудиторію та соц-граф, а реальну платіжку «вмикати» по ринках без ризику для ядра.

---

## 6. Послідовність впровадження (фази)

> Процес НЕ-узгоджуваний (`docs/rules.md` §3/§12): **кожна** задача — `docs/backlog/TASK-###-*.md` (INTAKE→REFINED→ESTIMATED→READY) + `docs/specks/SDD-###-*.md` з `AC-1..n` + **`SEC-1..n`** + Test Matrix ПЕРЕД кодом. HIGH/IMPOSSIBLE декомпозувати в LOW-листя (§12.5/§12.6). Container-first, 100% coverage, positive+negative+[SEC], `./scripts/ci.sh` зелений перед «done» (§8.6). Breaking API (напр. `validate` отримує required `event_id`) — узгодити версіонування клієнта (§10).

### Фаза 0 — Seam без поведінкових змін (продукт runnable)
| TASK / SDD | Зміст | Залежності |
|---|---|---|
| TASK-PROV-1 | `trait PaymentProvider` + `NoopProvider` + `TestProvider` + registry; **без** дотику LS | — |
| TASK-LEDGER-1 | `ledger_entries` міграція + запис при mint/sweep/payout/refund | PROV-1 |
| SEC-вектори | Noop/Test не рухають реальних грошей; ledger immutable | |

### Фаза 1 — Продукт-ядро (P0, без реального PSP)
| TASK / SDD | Зміст |
|---|---|
| TASK-TIERS-1 | `ticket_tiers` + міграція дефолтного tier + create/edit UI + checkout `{tier_id, quantity}` |
| TASK-CHECKOUT-1 | `checkout()` → `provider_action`; прибрати `payTest`-coupling; промокод UI; зведення |
| TASK-AUTH-1 | Google-only Sign-In (Android+Web); прибрати email/password |
| TASK-QR-1 | Rotating QR + прибрати сирий токен + keep-screen-on |
| TASK-STAFF-1 | `event_staff` + `require_perm` + staff endpoints + event-scoped `validate` + Door Mode + staff home |
| TASK-FEED-1 | Cursor-feed + ранжування + таби; RSVP/save як feed-об'єкти |
| TASK-REVIEW-1 | Verified-attendee gate + бейдж |
| TASK-BLOCK-1 | Block/Mute + фільтрація контенту |
| TASK-DISCOVER-1 | Параметризований `GET /api/events` + пошук+фільтри (Android+Web), web SSR |

### Фаза 2 — Operator back-office реальний (P0–P1)
| TASK / SDD | Зміст |
|---|---|
| TASK-ADMIN-FIN-1 | Dashboard KPIs + timeseries + escrow exposure |
| TASK-ADMIN-ORD-1 | Order/dispute detail drawer + dispute reason |
| TASK-ADMIN-SEAM-1 | Admin refund/release/payout через `provider.*` |
| TASK-RECON-1 | `/finance` reconciliation + CSV export |
| TASK-PAYOUT-1 | Organizer payout-request + bulk payout run |

### Фаза 3 — Соц-глибина + worldwide-готовність (P1)
| TASK / SDD | Зміст |
|---|---|
| TASK-SOCIAL-2 | Медіа в постах/відгуках/DM + Followers/Following + композер + invite/share |
| TASK-NOTIF-1 | Соц-сповіщення deep-link+групування+аватари; push opt-in |
| TASK-KYC-1 | KYC/KYB queue + reason; TASK-RISK-1 anti-fraud signals |
| TASK-ONBOARD-1 | DOB-крок + follow-suggestions; analytics-воронка |

### Фаза 4 — Реальний PSP (вмикається коли продукт уже живий)
| TASK / SDD | Зміст |
|---|---|
| TASK-PROV-LS-1 | `LemonSqueezyProvider` за trait; generic `/api/webhooks/:provider`; видалити LS-гілки з домену |
| TASK-PROV-N | `StripeConnectProvider` / `LiqPayProvider` / … — лише impl trait, **нуль** змін у ticketing/escrow |
| TASK-PAY-UI-1 | Provider-agnostic payment-settings UI + readiness |
| SEC | live-mode вимагає всіх creds; webhook HMAC per-provider; payout KYC-gate незмінний |

**Інваріант усіх фаз:** ticketing + escrow домен provider-neutral; реальна платіжка приходить у Фазі 4 і не торкається коду, написаного у Фазах 0–3.