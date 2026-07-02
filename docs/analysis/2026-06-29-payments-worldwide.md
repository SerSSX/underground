# Платіжна архітектура Underground — worldwide прийом, escrow та виплати організаторам

> Версія документа: 1.0 · Дата: 2026-06-29 · Авторство: principal payments/product
> Контекст: двосторонній **ескроу-маркетплейс квитків** (будь-хто створює подію та продає квитки; будь-хто купує). Гроші утримуються до завершення події, потім виплачуються організатору. Уся грошова логіка — через єдиний seam `api/src/escrow.rs` (+ `orders.rs`, `payouts.rs`, `admin.rs`, `webhooks.rs`).
> Поточний код використовує **Lemon Squeezy (MoR)**, який **забороняє** ticket-маркетплейс із виплатами третім сторонам — його замінюємо.

---

## 1. Резюме та головна рекомендація

**PRIMARY: Stripe Connect (Express accounts, separate charges & transfers) на US Delaware C-corp через Stripe Atlas** — як єдиний acquirer + escrow + payout seam для MVP і виходу worldwide. Stripe дає найширший у світі buyer-side (картки, Apple/Google Pay, Link, 100+ локальних методів, 195+ країн pay-in), а патерн *separate charges & transfers* лягає 1-в-1 на вашу наявну FSM у `api/src/escrow.rs`: charge зараховує кошти **на platform balance** (custody під ліцензією банків-партнерів Stripe — ви **не** стаєте money transmitter), а Transfer організатору викликається **явно** (як ваш `payouts::create_payout`). Sandbox — найкращий безкоштовний test mode без юр-особи й без KYC: повний escrow-флоу (charge → hold → transfer → payout → refund → dispute) будується **вже сьогодні**, до інкорпорації. Stripe Atlas (Delaware C-corp, ~$500) — канонічний, перевірений шлях для UA-засновника.

**FALLBACK / ДОПОВНЕННЯ (двошарова стратегія, вмикати ОДРАЗУ як другий шар):**
- **Payout-fallback — Payoneer Mass Payouts** для організаторів у країнах поза досяжністю Stripe Connect (~46 країн): 190+ країн / 70 валют, безкоштовний sandbox, бере KYC/AML payee на себе, і — критично — **найкраще виплачує українським організаторам** (вивід на банк ~1%, Monobank ~2%), чого Stripe Connect не вміє взагалі (UA не є payout-destination). Альтернатива для ширини+tax: **Trolley** (210+ країн, 135 валют, free forever-sandbox, вбудований KYC+tax widget; мінус — мінімум 100 виплат/міс і 4-6 тижнів активації rails).
- **Повна заміна провайдера (якщо Stripe заблокує escrow-aggregation або команда обере EU/UK)**: **Mangopay** — найбільш escrow-native (ліцензований EMI, e-wallets з **необмеженим** hold-періодом). Блокери: entity MUST бути EEA/UK (Atlas/Delaware не підходить), payout-мережа вузька (SEPA+US/CA). Якщо цей шлях — інкорпоруватись як **UK Ltd** (покриває одночасно Stripe + Mangopay + Adyen).
- **Rapyd** — резервний one-stop (acquirer+wallet+disburse 190+ країн), ймовірно приймає UA-entity, free sandbox; мінус — непрозорий pricing і менш «продуктизований» escrow. Тримати як план Б.

**Чому саме Stripe Connect:** ваш реальний код (`escrow.rs`/`payouts.rs`/`orders.rs`/`admin.rs`) уже реалізує admin-controlled escrow з indefinite hold (sweep `held→clearing→cleared`; payout лише явним KYC-gated admin-викликом; гілки `frozen`/`refunded`). Це provider-agnostic seam — потрібен провайдер, який (а) тримає кошти у своїй custody, (б) релізить лише за явним API-викликом, (в) глобально приймає від покупців, (г) дає безкоштовний sandbox без юр-особи. Stripe Connect задовольняє всі чотири й має найкращі acceptance+sandbox+UA-онбординг. Єдина слабкість — payout reach (~46 країн, без України) — точково закривається payout-fallback, а `escrow.rs` лишається seam, де `PAID_OUT` маршрутизується або в Stripe Transfer, або в Payoneer payout залежно від країни організатора.

---

## 2. Вимоги Underground до платіжного стеку

| # | Вимога | Що це означає на практиці | Хто закриває |
|---|--------|---------------------------|--------------|
| 1 | **Worldwide acceptance** | Покупець із будь-якої країни платить карткою / Apple Pay / Google Pay / локальним методом | Stripe (195+ країн pay-in, 100+ методів) |
| 2 | **Escrow-hold** | Гроші лежать у custody провайдера, а не на банк-рахунку Underground; реліз — лише за явним admin-викликом після події | Stripe platform balance (separate charges & transfers) / Mangopay e-wallet / Rapyd wallet |
| 3 | **Payout організаторам worldwide** | Виплата третій стороні (організатору) у багатьох країнах, включно з Україною | Stripe Connect (~46 країн) + **Payoneer/Trolley** для решти й для UA |
| 4 | **Безкоштовний sandbox без юр-особи** | Побудувати й протестувати весь флоу до інкорпорації | Stripe test mode, Payoneer sandbox, Trolley/Mangopay/Rapyd sandbox |
| 5 | **Не стати money transmitter** | Underground ніколи не тримає чужі кошти на власному банк-рахунку | Кошти завжди на provider balance; `escrow.rs` лише викликає Transfer/Refund |
| 6 | **Сумісність з UA-засновником** | Робоча юр-особа, доступна команді з України | Stripe Atlas (Delaware C-corp) або UK Ltd / Estonia OU |
| 7 | **AUP дозволяє ticket-marketplace** | Модель не заборонена провайдером (на відміну від Lemon Squeezy MoR) | Stripe Connect = організатор merchant-of-record своїх квитків |

---

## 3. Порівняльна таблиця провайдерів

| Провайдер | Worldwide-прийом | Escrow-hold | Payout-reach | Sandbox | UA-засновник | Fee (орієнтовно) | Оцінка |
|-----------|------------------|-------------|--------------|---------|--------------|------------------|--------|
| **Stripe Connect Express (Atlas US)** | Найкраща: cards+Apple/Google Pay+Link+100+ local, 195+ країн | Native: separate charges & transfers, кошти на platform balance; hold ~90 днів (US до 2 років) | ~46 країн + Global Payouts; **БЕЗ України** | Найкращий: free, без entity/KYC, повний Connect-флоу | Через Atlas (Delaware C-corp ~$500); UA напряму ні | 2.9%+$0.30 + $2/міс/акаунт + 0.25%+$0.25/payout | **5** |
| **Mangopay (EMI, EEA/UK)** | Сильна у Європі (SEPA/iDEAL/Bancontact); слабша поза EU | Найкращий native: e-wallets, **необмежений** hold; EMI custody | Вузька: EEA+UK (SEPA) + US/CA | Free self-serve, без entity | **Блокер**: entity MUST EEA/UK; Atlas не підходить | від ~1.4%+€0.25; platform fee ~€249/міс | **4** |
| **Rapyd (acquirer+disburse)** | Дуже широка: 900+ local у 100+ країн | Wallet-based hold без жорсткого ліміту; більше ручної логіки | **Найкраща**: 190-200+ країн, 120+ валют | Free self-serve, KYB лише для prod | Ймовірно приймає UA-entity (підтвердити KYB) | Непублічний, custom quote | **4-5** |
| **Airwallex (Connected Accounts)** | 170+ валют, 160+ методів | Wallet hold + payout timing | Широка: 150-200+ країн, 170+ валют | **Справжній self-serve** за 2-3 хв | Ймовірно приймає UA-entity | ~1.65%+$0.30 domestic / ~3.4% intl; FX 0.5-1% | **4-5** |
| **Adyen for Platforms** | Топова enterprise + local | Native balance accounts, hold до явного payout | Вузька: ~33-35 країн EEA/UK/CH | Test є, але Platforms-фічі sales-gated | Enterprise-only; UA не підтримується | Interchange++ + monthly minimum | **2-3** |
| **Checkout.com (Integrated Platforms)** | Сильний acquiring, високі auth-rates | Відкладений payout (не окремий escrow-wallet) | EU/UK/US-центрична | Free для card; platform-фічі sales-gated | EU/UK entity; UA напряму малоймовірно | Custom enterprise | **3** |
| **PayPal Complete Payments (Multiparty)** | Дуже сильна: 200+ ринків | delayed disbursement **МАКС 28 ДНІВ** — ламає post-event escrow | 200+ ринків (PayPal-акаунти) + Hyperwallet | Free, авто buyer/seller акаунти | **Блокер**: PayPal в UA лише SEND | ~2.59-3.49%+fixed | **3** |
| **Braintree Marketplace** | Сильний card gateway | Legacy escrow — **продукт ЗАКРИТО для нових** (лист. 2024) | US-only (історично) | Free, але deprecated | Недоступний UA | n/a | **1** |
| **Payoneer Mass Payouts** (payout-шар) | НЕ acquirer | Тримає в Mass Payout balance; escrow-логіка на вашому боці | 190+ країн, 70 валют; **найкраще в Україну** | Free sandbox (developer.sandbox.payoneer.com) | **Висока**: легально в UA і для платформи, і для payee | Per-payout (sales); UA вивід ~1%, Mono ~2% | **4** |
| **Trolley** (payout-шар) | НЕ acquirer | Payout-only; ескроу тримаєте ви | 210-220+ країн, 135 валют | **Free forever-sandbox** | EU/UK/US entity; виплати в UA можливі | $49/міс base + ~$10/wire + FX 1.9-3.5% | **4** |
| **Wise Platform** (payout-шар) | Частково (regional) | Multi-currency hold (не робить escrow/split) | 70+ країн local currency | Free sandbox | **Слабка UA**: немає виплат на UA БІЗНЕС-акаунти | Mid-market FX + низька комісія | **3** |
| **Tipalti** (payout-шар) | НЕ acquirer | Вбудований escrow + virtual IBANs | 200+ країн, 120 валют | Sandbox НЕ self-serve (через менеджера) | Enterprise; UA напряму малоймовірно | Custom, дорого | **2** |

Джерела: <https://docs.stripe.com/connect/separate-charges-and-transfers> · <https://docs.stripe.com/connect/cross-border-payouts> · <https://docs.stripe.com/sandboxes> · <https://stripe.com/atlas> · <https://mangopay.com/> · <https://docs.mangopay.com/guides/users/country-restrictions> · <https://www.rapyd.net/products/disburse/> · <https://www.airwallex.com/docs/payments-for-platforms/overview> · <https://docs.adyen.com/platforms> · <https://www.checkout.com/products/integrated-platforms> · <https://developer.paypal.com/docs/multiparty/checkout/delayed-disbursement/> · <https://www.payoneer.com/marketplace/mass-payouts-platform/> · <https://trolley.com/platform/global-payout-network/>

---

## 4. Детальний розбір топ-кандидатів

### 4.1 Stripe Connect Express — PRIMARY (5/5)

**Модель маркетплейсу.** Connected accounts (Express): Stripe-hosted onboarding + lightweight dashboard; платформа програмно контролює payout schedule. Заряди: *destination charges* або *separate charges and transfers*. Для escrow обираємо **separate charges and transfers** — charge зараховує кошти на **platform balance**, де вони фізично лежать = ваш стан `held`; Transfer на connected account викликається явно ПІСЛЯ події.

**Як тримати кошти до події.** Два механізми, обидва підходять:
1. **separate charges & transfers** (рекомендовано): charge без `transfer_data`; кошти на platform balance; після події `POST /v1/transfers` на connected account.
2. **manual payouts** (`payouts.schedule.interval=manual`) на connected account — кошти лежать на балансі connected account до явного Payout.
Hold-вікно: ~90 днів для більшості країн; **US-платформа — до 2 років** (критично для далеких фестивалів); reserve release максимум 180 днів.

**Payout-охоплення.** ~46 повністю підтримуваних країн (груд. 2025) + Global Payouts (нові країни груд. 2025 / січ. 2026) + Paystack для 5 африканських країн. Self-serve cross-border між US/UK/EEA/CA/CH. **Україна не є payout-destination** — закривається через Payoneer/Trolley.

**Fees.** 2.9% + $0.30 (US-картки; EU ~1.5%+); Express додає **$2/міс за КОЖЕН active connected account** + **0.25% + $0.25 за КОЖЕН payout**; cross-border transfer 0.25% (0% within EEA); FX ~1-2%; reserves 10-25% rolling для high-risk.

**Sandbox.** Найкращий: free test mode, без entity/KYC, повний Connect-флоу включно з hosted onboarding (фіктивні KYC-дані, тестові tax-id), test cards, transfers, refunds, disputes.

**AUP щодо квитків.** Event ticketing **не** у prohibited-списку. КРИТИЧНО: «payment facilitation / aggregation for third-party sellers» — RESTRICTED, але **Connect — санкціонований Stripe спосіб** робити third-party payouts (організатор = merchant-of-record). Resale/secondary — elevated risk, потрібен окремий апрув. Delayed-delivery ticketing — high-chargeback профіль → очікуйте reserves.

**Плюси:** найкраща acceptance + найчистіший escrow seam + найкращий sandbox + Atlas для UA + вбудований KYB/KYC; рекомендований дефолт Stripe для нових маркетплейсів.
**Мінуси:** UA не payout-destination → обов'язковий payout-fallback; $2/міс+per-payout на масштабі; ~90-денний hold (non-US); aggregation = restricted (ризик заморозки при неправильному описі моделі).

Джерела: <https://docs.stripe.com/connect/separate-charges-and-transfers> · <https://docs.stripe.com/connect/manual-payouts> · <https://docs.stripe.com/connect/connected-account-reserves> · <https://stripe.com/legal/restricted-businesses> · <https://docs.stripe.com/connect/testing>

### 4.2 Mangopay — найбільш escrow-native FALLBACK (4/5)

**Модель.** Wallet-native PSP: кожен користувач отримує e-wallet. Pay-in → wallet → wallet-to-wallet transfers (split: комісія платформи + частка організатора) → payout (cash-out) на банк.

**Як тримати кошти.** Native: ліцензований EMI тримає e-money у wallet **необмежено довго** — ви самі визначаєте затримку між pay-in і payout. Ідеально для далеких подій-фестивалів. Underground custody не несе → money-transmitter ліцензія не потрібна.

**Payout-охоплення.** Вузьке: SEPA-зона (EUR) + US/CA/DK через Currencycloud; 15 валют payout. Це головний мінус для worldwide.
**Fees.** Custom; історично від ~1.4%+€0.25 pay-in, payout від ~€0.20, platform fee ~€249/міс.
**Sandbox.** Free self-serve, без entity; всі production-фічі + Postman-колекція.
**AUP.** Ticketing не prohibited; «payment escrow services» — restricted (enhanced DD, case-by-case) → потрібне явне погодження.
**Блокери:** entity MUST бути EEA/UK (Atlas/Delaware не підходить); вузька payout-мережа.

Джерела: <https://docs.mangopay.com/guides/e-wallet-system> · <https://docs.mangopay.com/guides/users/country-restrictions> · <https://mangopay.com/prohibited-businesses>

### 4.3 Rapyd — резервний one-stop (4-5/5)

**Модель.** Collect + Wallet + Disburse в одному. Per-user wallets для hold; split через wallet transfers + payout API.
**Як тримати кошти.** Wallet-based hold без жорсткого ліміту днів (контролюєте ви).
**Payout-охоплення.** **Найкраще у світі**: 190-200+ країн, 120+ валют (bank/RTP/card/wallet/stablecoin).
**Acceptance.** Найширші local-методи: 900+ у 100+ країн.
**Sandbox.** Free self-serve (dashboard.rapyd.net), KYB лише для prod (review до 30 днів).
**UA.** Ймовірно приймає UA-entity (підтвердити при KYB) — менше потреби міняти юрисдикцію.
**Мінуси:** непрозорий pricing; escrow менш продуктизований (більше ручної wallet-логіки); ticketing/escrow можуть тригерити enhanced DD.

Джерела: <https://www.rapyd.net/products/disburse/> · <https://docs.rapyd.net/en/supported-currencies.html> · <https://dashboard.rapyd.net/sign-up>

### 4.4 Payoneer Mass Payouts — обов'язковий payout-шар (4/5)

**Роль.** НЕ acquirer — лише payout rail + сховище balance. Паруєте зі Stripe (acceptance).
**Як тримає кошти.** Mass Payout balance (до 30 валют); виплата лише за вашою інструкцією — ескроу-логіка у `escrow.rs`.
**Payout-охоплення.** 190+ країн, 70 валют; Payoneer account (хвилини) / local bank (до 3 днів) / eWallet. **Найкраще виплачує в Україну** (вивід ~1%, Monobank ~2%; з жовт. 2025 internal transfer $3 або 0.6%).
**UA.** Висока: легально працює в Україні і для платформи, і для payee; UA-компанія може відкрити Mass Payout account.
**Sandbox.** Free (developer.sandbox.payoneer.com); Payoneer бере KYC/AML payee на себе.
**Мінуси:** не acquirer; тарифи через sales; split-логіка на вашому боці.

Джерела: <https://www.payoneer.com/marketplace/mass-payouts-platform/> · <https://developer.sandbox.payoneer.com/psd2/home> · <https://www.payoneer.com/resources/money-transfers-to-ukraine-payment-platforms-and-commission-features/>

### 4.5 Чому НЕ PayPal Multiparty як основний escrow

`disbursement_mode=DELAYED` тримає кошти **максимум 28 днів**, після чого PayPal **автоматично** виплачує продавцю. Для квитків, що продаються за місяці до події, це ламає модель post-event escrow. Плюс PayPal в Україні — лише SEND. Залишається сильним кандидатом для acceptance, але слабкий саме на безлімітному escrow.

Джерело: <https://developer.paypal.com/docs/multiparty/checkout/delayed-disbursement/>

---

## 5. Юрисдикція: де реєструвати компанію

### Чому тримання чужих коштів ≠ ваша ліцензія (як цього уникнути)

**Money-transmitter ризик виникає, якщо ВИ тримаєте кошти на власному банк-рахунку Underground.** Архітектурне рішення: кошти у фазі `HELD` фізично лежать **на балансі провайдера** під ЙОГО ліцензією (Stripe — через банки-партнери; Mangopay — EMI; Adyen — банк/PI). Underground лише **викликає API** (Transfer/Refund) після події. Тоді ви — technology platform, а не custodian. Уся логіка вже ізольована у `api/src/escrow.rs` — там мають жити виклики Connect Transfers / reserve-plans, **а не власне зберігання балансів**.

### Порівняння юрисдикцій

| Юрисдикція | Розблоковує | Блокує | Вартість/нюанси |
|------------|-------------|--------|------------------|
| **US Delaware C-corp (Stripe Atlas)** — РЕКОМЕНДОВАНО | Stripe Connect (найкраща acceptance+escrow) + PayPal + Payoneer; **hold до 2 років**; повний self-serve cross-border US/UK/EEA/CA/CH | Mangopay, Adyen-EEA (вони MUST EEA/UK) | ~$500 one-time; потрібні US-банкінг (Mercury/партнери), US-tax (form **5472/1120** для foreign-owned C-corp, ~$1.5-3k/рік), registered agent |
| **UK Ltd** — найгнучкіший single-entity | **Stripe + Mangopay (UK FCA EMI) + Adyen одночасно** | — | Найкращий, якщо стратегічно хочете Mangopay-escrow |
| **Estonia OU (e-Residency)** | Mangopay (CSSF) + Adyen + Stripe-EU; дружній до UA-засновників віддалено | Дещо вужча acceptance ніж US | Зручна податкова структура |
| **Чиста UA-юрособа** | — | **Блокує Stripe, PayPal-receive, Mangopay, Adyen напряму** | Не робити для acquiring |

**Висновок:** для worldwide-rails із найкращим провайдером — **Delaware C-corp через Atlas** (Stripe Connect). Якщо стратегічно обираєте Mangopay-escrow або EU-присутність — **UK Ltd** (покриває й Stripe, і Mangopay, і Adyen). **Розробку можна почати вже зараз у sandbox без жодної entity**; інкорпорацію (Atlas) робити паралельно перед live.

Джерела: <https://stripe.com/atlas> · <https://docs.stripe.com/atlas/signup> · <https://support.mangopay.com/s/article/In-which-countries-should-my-company-be-registered-in-to-use-MANGOPAY> · <https://financialit.net/news/payments/mangopay-granted-uk-e-money-license>

---

## 6. Мапінг на `api/src/escrow.rs`

Маппінг на **реальну FSM у коді** (Stripe Connect, separate charges & transfers). Seam лишається `escrow.rs` + `orders.rs` + `payouts.rs` + `admin.rs` + `webhooks.rs`. Підтверджено читанням коду:
- `escrow.rs:10` — `sweep(db, clearing_days)`: `UPDATE ... escrow_status='clearing' WHERE escrow_status='held'` (після `ends_at`); потім `'clearing'→'cleared'`.
- `payouts.rs:31` — `create_payout(...)`: перевіряє `SELECT blocked, kyc_status FROM users` (KYC-gate), потім `UPDATE orders SET escrow_status='paid_out' WHERE escrow_status='cleared'`.

| Стан FSM | Тригер у коді (зараз) | Дія у Stripe (нова) |
|----------|----------------------|---------------------|
| **HELD** | `orders::fulfill` → `escrow_status='held'`; зараз ставиться у `webhooks.rs`/`pay_test` після Lemon Squeezy `order_paid` | `PaymentIntent` з capture на **PLATFORM balance** (charge без `transfer_data`, без `application_fee_amount` на цьому етапі). Кошти на platform balance = `held`. Webhook **`payment_intent.succeeded`** замінює Lemon Squeezy `order_paid` і викликає `orders::fulfill` |
| **CLEARING** | `escrow::sweep` → `'held'→'clearing'` після `events.ends_at` (`escrow.rs:12-14`) | Чисто **внутрішній** стан ledger — **жодного Stripe-виклику**. Кошти продовжують лежати на platform balance. `sweep` лишається як є |
| **CLEARED** | `escrow::sweep` → `'clearing'→'cleared'` після `clearing_window_days` (`escrow.rs:23-25`); або admin release | Внутрішній стан «готово до Transfer». **Жодного Stripe-виклику**. Опційно: для high-risk ticketing — connected-account reserve plan з date-based release (наступний день після події) |
| **PAID_OUT** | `payouts::create_payout` (admin, KYC-gated; `payouts.rs:31,38,51-53`) | **Тут і ТІЛЬКИ тут** рух грошей = **Transfer API** (`POST /v1/transfers`, `amount` = нетто комісії = ваш `round(...*(10000-bps)/10000)`, `destination`=organizer connected account). Опц. далі Payout на банк. Для організаторів **поза ~46 країнами Stripe** — замість Transfer викликати **Payoneer/Trolley** payout API. KYC-gate (`kyc_status='verified'`) мапиться на Connect `charges_enabled`/capability |
| **DISPUTED** | `orders::dispute` / admin freeze → `'frozen'` | Двосторонньо: (а) chargeback покупця в банку → webhook **`charge.dispute.created`** → код ставить `'frozen'`; (б) internal dispute → лишається `'frozen'`, Transfer не викликається. Поки `'frozen'` — Transfer guard блокує виплату |
| **REFUNDED** | admin `escrow_action='refund'` → `orders.status='refunded'`, повернення ticket/seat/promo | **Refund API** (`POST /v1/refunds` на PaymentIntent). Якщо кошти вже transfer-нуті — спершу **Transfer Reversal** (`POST /v1/transfers/:id/reversals`), потім Refund. Наявна idempotent-логіка (`rows_affected` guard) лишається |

**Ключове:** код уже моделює ledger правильно. Stripe-виклики додаються **лише на двох межах** — вхід грошей (`HELD` = charge) і вихід (`PAID_OUT` = Transfer, `REFUNDED` = Refund/Reversal, `DISPUTED`-chargeback = webhook). `CLEARING`/`CLEARED` лишаються чисто внутрішніми. У `payment_settings` додати `provider='stripe'` гілку **паралельно** до `'lemonsqueezy'` за provider-флагом — НЕ ламати наявний шлях.

Джерела: <https://docs.stripe.com/connect/separate-charges-and-transfers> · <https://docs.stripe.com/connect/marketplace/tasks/refunds-disputes> · <https://docs.stripe.com/connect/connected-account-reserves>

---

## 7. Монетизація / собівартість

Ефективна вартість на транзакцію (Stripe Connect Express, US-платформа):

| Компонент | Ставка |
|-----------|--------|
| Acquiring (US-картки) | 2.9% + $0.30 (EU-картки ~1.5%+) |
| Connect Express overhead | **$2/міс** за КОЖЕН active connected account (організатор з payout того місяця) |
| Payout fee | 0.25% + $0.25 за КОЖЕН payout |
| Cross-border transfer | 0.25% (0% within EEA / UK↔EEA) |
| FX | ~1% (до 2% для non-USD у деяких регіонах) |
| Reserves (не «вартість», а cash-flow gap) | rolling 10-25% (release ~90 днів) АБО date-based (release наступного дня після події) |

**Приклад (квиток $50, US-buyer, US-organizer):** acquiring 2.9%×50+0.30 = **$1.75**; payout 0.25%×X+$0.25 ≈ **$0.37**; +$2/міс на акаунт (амортизується на обсязі). Ефективно **~4-5% на низькому обсязі**, падає до **~3.3-3.5% на обсязі**.

**Порівняння.** Stripe Standard accounts прибрали б $2/міс+per-payout (дешевше), але організатор бачить Stripe-брендинг і володіє disputes. Payoneer-fallback додає ~1-2% на UA/exotic payout. Mangopay/Adyen — непрозорий custom pricing + місячний мінімум (~€249 Mangopay / enterprise-invoice Adyen) — дорожче на старті. **Висновок: Stripe Express — найпередбачуваніша вартість для ранньої стадії без sales-перемовин.**

Джерело: <https://docs.stripe.com/global-payouts/pricing> · <https://docs.stripe.com/connect/cross-border-payouts>

---

## 8. План «ТЕСТУВАТИ ВЖЕ ЗАРАЗ» (без реальної компанії)

| Крок | Дія | Деталі / посилання |
|------|-----|---------------------|
| 1 | Завести **безкоштовний Stripe account** (без юр-особи), переключитись у Sandbox/test mode | <https://docs.stripe.com/sandboxes> → test keys `sk_test_...`/`pk_test_...` |
| 2 | Увімкнути **Connect** (test mode); створити 2-3 test **Express** connected accounts через Accounts API; згенерувати Account Links → пройти hosted onboarding з фіктивними KYC-даними | <https://docs.stripe.com/connect/testing> · <https://docs.stripe.com/connect/testing-verification> (мапиться на `kyc.rs` + payout KYC-gate) |
| 3 | Додати у `payment_settings` гілку `provider='stripe'` (поряд з `'lemonsqueezy'`); скласти Stripe Rust client (**async-stripe** crate) за seam-патерном, **паралельно** наявному шляху | `api/src/payments.rs`, `webhooks.rs` |
| 4 | **HELD**: реалізувати PaymentIntent на platform balance, оплатити test-карткою **`4242 4242 4242 4242`**; підтвердити, що webhook `payment_intent.succeeded` → `orders::fulfill` → `escrow_status='held'` | <https://docs.stripe.com/connect/testing> |
| 5 | **CLEARING/CLEARED**: прогнати `escrow::sweep` вручну (підкрутити `events.ends_at` у минуле через міграцію/seed у test БД, **НЕ** прямою мутацією prod) — переконатись `held→clearing→cleared` без Stripe-викликів | `escrow.rs:10` |
| 6 | **PAID_OUT**: викликати `payouts::create_payout` → Transfer API на test connected account; перевірити нетто-суму комісії і що KYC-gate блокує неверифікованого організатора | `payouts.rs:31` |
| 7 | **DISPUTED**: (а) симулювати chargeback карткою **`4000 0000 0000 0259`** → `charge.dispute.created` → `'frozen'`; (б) перевірити internal `orders::dispute` → `'frozen'` і що Transfer не йде | — |
| 8 | **REFUNDED**: admin refund → Refund API; для вже-transfer-нутого — Transfer Reversal потім Refund; перевірити повернення ticket/seat/promo (idempotent) | — |
| 9 | **ПАРАЛЕЛЬНО**: завести **Payoneer sandbox** — створити test payees, викликати Mass Payout API; змоделювати PAID_OUT-гілку для організатора поза Stripe-reach (зокрема UA-організатор) | <https://developer.sandbox.payoneer.com/psd2/home> |
| 10 | Прогнати `./scripts/ci.sh` + `cargo test` з **[SEC]**-векторами (підроблений webhook signature, payout без KYC, повторний refund, dispute чужого order) перед «done» | CLAUDE.md §8.6 |

**Тестові картки Stripe:** успіх — `4242 4242 4242 4242`; chargeback/dispute — `4000 0000 0000 0259`; 3DS — `4000 0027 6000 3184`. Все це **без реальної юр-особи й без KYC**.

---

## 9. Ризики та відкриті питання для засновника

### Ризики (з рекомендаціями)

| Ризик | Опис | Рекомендація |
|-------|------|--------------|
| **AUP / account-freeze** | Ticket-marketplace з aggregation для third-party організаторів — RESTRICTED у Stripe | **Явно описати модель Stripe** (Connect = організатор merchant-of-record). Підтвердити зі Stripe sales перед масштабуванням, особливо для resale |
| **Payout reach gap** | Stripe НЕ виплачує в Україну і ще ~150 країн | **Payout-fallback (Payoneer/Trolley) — обов'язковий другий шар з дня 1**, не «колись потім» |
| **Reserves / cash-flow** | Delayed-delivery ticketing = high-chargeback → 10-25% rolling reserve або date-based hold | Закласти у cash-flow модель заздалегідь |
| **Hold-cap** | Manual payout ~90 днів (non-US) / reserve release max 180 днів | Для фестивалів з продажем за 6-12 міс — **US-platform через Atlas (до 2 років) обов'язкова** |
| **UA-entity неможлива** | Інкорпорація Atlas + US-tax (5472/1120) + банкінг = overhead | Прийняти як ціну входу або обрати UK Ltd / Estonia OU |
| **Money-transmitter** | Якщо тримати кошти на власному рахунку Underground — потрібна ліцензія | Кошти **завжди** на provider balance; `escrow.rs` лише викликає Transfer/Refund, **не** зберігає баланси |
| **Provider lock-in у `webhooks.rs`** | Код жорстко прив'язаний до Lemon Squeezy (HMAC X-Signature, `order_paid`) | Міграція за provider-флагом у `payment_settings`, інакше ламається наявний шлях і CI |

### Відкриті питання

1. **Який реальний lead-time подій?** Якщо квитки продаються >90 днів до події → US-platform через Atlas (до 2 років hold) **обов'язкова**; якщо <90 днів — non-US варіанти теж життєздатні.
2. **Чи буде resale/secondary marketplace?** Це high-risk у ВСІХ провайдерів і потребує окремого письмового апруву — primary-продаж значно простіший.
3. **Підтвердити зі Stripe sales:** чи апрують escrow ticket-marketplace з third-party payouts на Atlas Delaware C-corp **до** того, як вкладати тижні в інтеграцію.
4. **Чи готова команда до US-tax compliance** (form 5472/1120, ~$1.5-3k/рік)? Якщо ні — UK Ltd / Estonia OU простіші податково.
5. **У яких ТОП-5 країнах будуть організатори?** Визначає, чи Stripe ~46 країн достатньо, чи payout-fallback критичний з дня 1.
6. **Який очікуваний місячний обсяг payouts?** <100/міс **виключає Trolley** (мінімум 100/міс) → Payoneer єдиний payout-fallback.
7. **Чи потрібен повний white-label** (організатор не бачить Stripe)? Якщо так — Custom accounts (важче) або Mangopay/Rapyd wallet-модель.
8. **Підтвердити UA-eligibility Payoneer Mass Payout account** на UA-entity АБО на Atlas-entity — від цього залежить, як виплачувати UA-організаторам.

---

*Документ підготовлено на основі дослідження Stripe Connect, PayPal/Braintree, Adyen, Airwallex, Mangopay, Rapyd, Checkout.com, Nuvei, Payoneer, Wise, Trolley, Tipalti. Усі джерела наведені інлайн як URL. Релевантні файли коду: `/Users/serhii/Projects/underground/api/src/escrow.rs`, `/Users/serhii/Projects/underground/api/src/payouts.rs`, `/Users/serhii/Projects/underground/api/src/orders.rs`, `/Users/serhii/Projects/underground/api/src/webhooks.rs`, `/Users/serhii/Projects/underground/api/src/payments.rs`.*