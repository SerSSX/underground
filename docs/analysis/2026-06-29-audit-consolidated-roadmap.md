# Underground — Консолідований роадмап покращень (аудит PM/Marketing/Design)

> **Статус:** підсумковий консолідований документ за результатами аудиту в чотирьох дисциплінах (PM, Marketing, UX/Design, Cross-cutting).
> **Governing doctrine:** зміни поведінки фіксуються у `docs/spec.md` ПЕРЕД тестами й кодом (rules §0/§2). Детальні тех-завдання (SDD/AC/SEC/Test-Matrix) виводяться окремим файлом і зшиваються з ініціативами через `ID` (`INIT-*`).

---

## 1. Резюме та North Star

Underground будується як **популярний, простий і зручний** сервіс, що поєднує організацію івентів, персональну пропозицію подій, продаж/обмін/дарування білетів, перевірку на вході з делегуванням і базову соцмережність як двигун росту. Головні чесноти — **простота, зручність, віральність**; продукт свідомо **не перевантажується** зайвими фічами.

Аудит виявив системний розрив: **центральна віральна петля засновника** (перейшов за лінком без акаунта → забрав білет-подарунок → зареєструвався тут же в пару кліків → сам ділиться) **фізично обірвана** і рознесена по непов'язаних P1/P2-фрагментах. `transfer_ticket` (`api/src/tickets.rs:265`) падає, якщо email отримувача не зареєстрований; немає стану `unclaimed`, web-роутів `/g` `/t`, deep-link/autoVerify у Android-манифесті, вихідного share-CTA. Канали доставки (FCM `main.rs:139`, email `main.rs:172`) — TODO-заглушки. Наявний telemetry-seam не інструментований під воронку/K-factor. Паралельно repo містить **два взаємовиключні роадмапи** (money-first vs product-first) — порушення rules §0.

### North Star

| Рівень | Метрика | Визначення |
|---|---|---|
| **North Star** | Квитки на активного користувача / 28 днів | Paid + free/RSVP; поєднує активацію попиту, віральність і GMV, не караючи free-first старт |
| **Вхідна метрика росту** | **K-factor ≥ 0.5** | share/gift-лінк → claim → signup → re-share |
| **Supply-side контр-метрика** | payout-latency (SLA) | `ends_at → PAID_OUT`; алерт на застряглі кошти |
| **Пороги готовності релізу** | share→claim ≥ 30%, guest→signup ≥ 40%, K ≥ 0.5 | pruning-тест для P0 |

**Головне стратегічне рішення:** `product-first, payment last`. Перший реліз — **Simple Launch** на free/RSVP з `NoopProvider`; реальні гроші/ledger/compliance — після PMF (Фаза 4).

---

## 2. Принципи продукту (простота / зручність / віральність)

| # | Принцип | Суть |
|---|---|---|
| 1 | **Простота за замовчуванням, глибина за запитом** | Кожен ключовий екран (Афіша, create-event, checkout) стартує з ОДНОГО рішення; решта — за «Розширеними налаштуваннями». |
| 2 | **Цінність перед авторизацією** | Анонім бачить подію/деталі/подарунок ДО логіну; auth-wall — лише в момент дії (claim/buy/RSVP). |
| 3 | **Frictionless віральність — це продукт, а не фіча** | Gift/share-петля проєктується end-to-end на ОДИН signed-token seam; часткова реалізація = 0 K-factor → не робимо частково. |
| 4 | **Довіра видима покупцю** | Escrow — продающий trust-сигнал на поверхні, не бек-офіс; але зі supply-клапаном (early-payout, передбачуваність виплат). |
| 5 | **Все вимірюване** | Жодна віральна/конверсійна фіча не виходить без tracking-подій; tracking plan — дешеве розширення telemetry-seam, не новий екран. |
| 6 | **P0 = «блокує ПЕРШИЙ цінний реліз»**, а не «архітектурно важливо» | Усе, що не рухає activation/virality-метрику, — не P0. |
| 7 | **Product-first, payment last** | Реальні гроші/compliance/ledger — після PMF; старт на free/RSVP + NoopProvider. |
| 8 | **Frictionless ≠ діряво** | Кожен guest/gift-шлях обгорнутий anti-abuse (single-use short-TTL token + rate-limit + device/email-cap) як УМОВА готовності. |
| 9 | **Не будуємо другий Instagram** | Соц-шар = рівно friends-going + share + invite + gift; усе понад це — відкладаємо. |
| 10 | **Дешеві незворотні контракти фіксуємо зараз** | minor-units, event_timezone, i18n-каркас, start/end layout — щоб не робити breaking-міграції на живому продукті. |
| 11 | **Spec — єдине джерело істини** | Зміна поведінки: спершу `docs/spec.md`, потім тести, потім код. |

---

## 3. Ключові теми змін за дисциплінами (головні висновки аудиту)

### 3.1. PM — пріоритети, воронка, ліквідність

- **Головна віральна петля не спроєктована end-to-end** і стоїть P1/P2 замість P0 — найгостріший розрив між документами й ціллю засновника (**6 із 9 аудиторів згодні**).
- **Пріоритетна інверсія:** virality-critical речі (guest-claim, share, RSVP-соц-доказ, простий checkout) — P1, а фінансовий бек-офіс реальних грошей (ledger, reconciliation, KYC/KYB, generic webhook) — P0, попри свідомий free/RSVP-старт.
- **Немає North Star, воронки як вимірюваного об'єкта, event-spec, K-factor-інструментації** — «простоту» й «віральність» не довести й не оптимізувати.
- **Cold-start ліквідність не адресована** — план demand-first над порожнім Києвом; ризик ghost-town без supply-seeding.
- **Складність зростає замість спрощення** — create-event роздувається (обов'язкові tiers, sales-windows, KYC-онбординг): рух до Eventbrite замість Luma/Partiful.
- **Escrow-as-trust трактується як бек-офіс**, а не видима обіцянка й moat; бракує early-payout клапана.
- **Ticket exchange/resale-домен провалено** — неіменні квитки + безлімітний transfer = скальпінг-канал.
- **Два роадмапи в repo прямо суперечать** (порушення §0).

### 3.2. Marketing — канали, органіка, атрибуція

- **Центральна guest-віральна петля фізично обірвана** на Android+web (немає `/t`, `/g`, deep-links, вихідного share-CTA) → K-factor нового вузла ≈ 0.
- **Канали доставки не існують** — FCM і email — TODO-заглушки; усі retention/virality-петлі — no-op. **Hard-prerequisite**, а не паралельна P1-інфра.
- **Органіка вимкнена за замовчуванням** — `home = 'use client'` (не індексується), немає JSON-LD/лендінгів/OG-image route; каркас (sitemap/robots) готовий.
- **Нуль growth-інструментації** — немає K-factor, invite/gift-funnel, `?ref`-атрибуції, воронки з drop-off, channel-breakdown, ASO.
- **UA-fit принесений у жертву worldwide-архітектурі** — не названо day-1 локальний PSP, escrow-trust похований у backend, онбординг вимагає SWIFT.
- **Соц-scope роздутий** до «другого Instagram».
- **Retention майже відсутній як шар** — немає review-nudge, winback, abandoned-checkout, scarcity, throttle.

### 3.3. Design / UX — простота екранів, довіра, вхід

- **Головний віральний контур розсипаний по фрагментах** і не спроєктований end-to-end (**6 із 8 аудиторів** — P0).
- **Signup-wall перед цінністю** — анонім не бачить афішу до Google-логіну, хоча `discover.rs` вже віддає trending аноніму (діра переважно навігаційна).
- **Технічний блокер** — `transfer_ticket` падає для незареєстрованого email; немає `pending_claim`.
- **Google-only auth конфліктує з frictionless** на claim/guest — потрібен passwordless email-код ЯК ВИНЯТОК.
- **Перевантаження перших екранів** — онбординг 6 полів/3 екрани; Афіша 4 сегменти + 6 рядів фільтрів; create-event ~22 поля; door-staff 4 permission-чіпи.
- **Discovery — набір фільтрів, а не продукт відкриття** — «що робити сьогодні ввечері» ніде не first-class.
- **Дозволи питаються сирим OS-промптом** без контексту — спалюють одноразовий opt-in.
- **Door Mode:** rotating-QR (P0) фізично несумісний з offline; ідемпотентність `(qr_token, scanned_at)` не захищає від подвійного входу; сирий `Text(qrToken)` = витік секрету.
- **Відсутній візуальний/a11y-контракт** — токени не обов'язкові, вердикти лише кольором, empty/skeleton/inline-error не специфіковані.

### 3.4. Cross-cutting — безпека, гроші, вимірність, незворотні контракти

- **Frictionless-шляхи = ріст-двигун і фрод-вектор одночасно** — треба anti-abuse контур ДО релізу.
- **Довіра має стати UX** — escrow-захист і trust-бейджі в точці покупки; dispute/refund типізований і прозорий.
- **Нульовий вимірювальний шар** — telemetry-seam не згадано; 40+ фіч ризикують вийти неінструментованими.
- **Три незворотні worldwide-контракти дешеві зараз** — minor-units, event_timezone, i18n-каркас; + негайний баг наївного рендеру часу (substring → -2/-3 год для Києва).
- **Anti-fraud має бути inline-превенцією**, а не лише пасивним admin /risk-дашбордом; rotating QR без anti-passback = security theater.
- **Спрощення заради фокусу** — не будувати повний risk-console/KYB-для-всіх/важку A-B-платформу/FX/RTL зараз.

---

## 4. Пріоритизований роадмап (ініціативи)

> Пріоритет: **P0** = блокує перший цінний реліз · **P1** = наступна хвиля довіри/supply · **P2** = масштабування/back-office.

| ID | Назва | Проблема (стисло) | Рішення (стисло) | Пріор | Impact | Effort | KPI | Залежності |
|---|---|---|---|---|---|---|---|---|
| **INIT-ROADMAP-SPEC** | Зафіксувати «product-first, payment last» у spec | Два взаємовиключні роадмапи (money-first vs product-first) — порушення §0 | Рішення у `spec.md` ПЕРЕД TASK-ами; improvement-plan M1 → Фаза 4; у Фазі 0 лише `PaymentProvider` + `NoopProvider`; P0 = «блокує перший реліз» | P0 | high | S | delivery predictability; WIP на критичному шляху | — |
| **INIT-CONTRACTS** | Незворотні контракти: minor-units, event_timezone, i18n + фікс часу | `amount_minor:i64` vs `NUMERIC(12,2)`; немає timezone-колонки; Android `substring` → UTC (-2/-3 год); 286 хардкод-кирилиць | Канон minor-units i64 + одноточкова конвертація; `events.event_timezone` (IANA); фікс рендеру (`java.time`/`Intl`); каркас `values-en`; start/end guardrail | P0 | high | S | reconciliation-accuracy; no-show-rate; time-to-worldwide | — |
| **INIT-TICKET-UNCLAIMED** | Стан `unclaimed`/`pending_claim` + gift на незареєстрований email | `transfer_ticket:265` падає на невідомому email; немає `pending_claim` — подарунок людині без акаунта неможливий | `tickets.status=unclaimed` + `claim_token` (міграцією); transfer не вимагає user; дарувальнику — «Надіслано/Переглянуто/Забрано» + відкликати; зміна у spec ПЕРЕД кодом | P0 | high | M | частка gift→claim | INIT-ROADMAP-SPEC |
| **INIT-GIFTLINK** | Наскрізна frictionless gift/claim-петля на один signed-token seam | Петля розсипана, прив'язана до відомого email/@username → «хто перший забрав» неможливий; вихідного share-CTA нема | claimable gift-link → універсальний https; OG-багата landing `/g /t` (цінність ДО логіну, QR розмито); одна кнопка «Забрати» (Google one-tap / email-код на тій самій сторінці); обов'язковий вихідний share-CTA; комісія НЕ з подарованих/free | P0 | high | L | K-factor; частка активацій з gift-links | INIT-TICKET-UNCLAIMED, INIT-DEEPLINKS, INIT-CHANNELS, INIT-ANTIABUSE |
| **INIT-DEEPLINKS** | Верифіковані universal-links + OG-прев'ю для ВСІХ share-об'єктів | Manifest без deep-link scheme/autoVerify; OG лише для події; лінк у месенджер без апки не відкриється | Android autoVerify + `underground://`; `assetlinks.json` + AASA; усі лінки — universal https; кожен shareable-екран SSR + OG; динамічний opengraph-image route (1200×630) для event/gift/RSVP | P0 | high | M | share-link CTR; visit→signup | — |
| **INIT-CHANNELS** | Робочий FCM push + email як перший клас lifecycle-каналів | FCM (`main.rs:139`) і email (`main.rs:172`) — TODO; gift-delivery/receipt/reminder/magic-code — no-op | FCM HTTP v1 (OAuth2 SA-key); email за одним seam (transactional обов'язково); health-check у admin; підняти до P0 hard-prerequisite | P0 | high | M | delivery success rate; guest→registered; D7/D30 | — |
| **INIT-ANTIABUSE** | Anti-abuse контур навколо всього guest/gift-claim | Frictionless-шлях без TTL/single-use, rate-limit, device/email-cap → ботнет-фарм і крадіжка подарунка | single-use + short-TTL claim-token (окремий секрет від QR); rate-limit + cap; magic-code (код у листі); «забрано з іншого пристрою»; auto-revert по TTL; SEC-вектори; фічу не вмикати без контуру | P0 | high | M | K-factor без токсичного трафіку; fraud-loss | INIT-TICKET-UNCLAIMED |
| **INIT-BROWSE-AUTH** | Browse-before-auth + passwordless email-код у claim/guest | Навігація веде новий user→Onboarding; Google-only конфліктує з claim (десктоп/не-Google/чужий пристрій) | Афіша+деталі — перший екран аноніма; auth лише на дію; persist anon-intent → merge при логіні; passwordless email-код ЯК ВИНЯТОК у claim/guest; identity-merge guest→Google; виняток у spec + апрув засновника | P0 | high | M | activation холодного трафіку; TTFW; claim completion; D1 | INIT-ROADMAP-SPEC |
| **INIT-ONBOARDING** | Не-блокуючий онбординг ≤1 екран + contextual permission priming | 6 полів/3 екрани ДО першої події; push/гео сирим OS-промптом | Обов'язково 1 екран (категорії + місто, дефолт Київ); DOB — ленивий age-gate на 18+ checkout; гео/follow — контекстно; pre-permission priming; ввести TTFW | P0 | high | S | onboarding completion; activation; push opt-in; TTFW | INIT-BROWSE-AUTH |
| **INIT-ANALYTICS** | Tracking Plan v1 + воронка + K-factor атрибуція + north-star дашборд | telemetry-seam не в планах; емітиться лише термінальний ticket; немає NSM/воронки/K-factor | ~20 канонічних подій з property-схемою; server-side purchase-воронка; `share_token/referrer_id` + парні `*_sent↔*_accepted`; `GET /admin/telemetry/funnel` + timeseries; цілі релізу як pruning-тест | P0 | high | M | NSM/funnel drop%/K-factor/activation/D7-D30/channel ROI | — |
| **INIT-DOORSTAFF** | Делеговане door-scan (STAFF-1a) + безпечний QR + повні вердикти | Монолітний TASK-STAFF-1 весь P0 (§12.5); rotating-QR ⊥ offline; сирий `Text(qrToken)`; ідемпотентність не тримає double-entry | Декомпозиція: **1a** (event_staff + validate + інвайт-лінк «Контролер» — P0), 1b/1c/1d — P1/P2; signed stable token + серверний atomic anti-passback (first-write-wins по `qr_token/ticket_id`); повний набір вердиктів (колір+іконка+текст); refund прибрати з дверей | P0¹ | high | M | scan success; double-entry rate; door-throughput | — |
| **INIT-DISCOVERY** | Time-aware персональна Афіша замість плоского фільтр-екрана | «Що робити сьогодні ввечері» не first-class; `discover.rs` закопаний у чіп; cold-start не специфіковано | Дефолт — вертикаль секцій («Сьогодні ввечері поруч»→«Для тебе»→«Друзі йдуть»→«Вихідні»); фільтри → рядок швидко-чіпів + bottom-sheet; fallback-ланцюг (trending+near) — «Для тебе» ніколи не порожнє; time+distance у EventCard | P1 | high | M | card→detail CTR; D1; search-success | — |
| **INIT-TRUST-ESCROW** | Escrow-as-trust видимий + dispute/refund workflow + early-payout | Escrow похований (лише checkbox); dispute — заглушка без SLA/типів; відлякує supply без early-payout | Бейдж «Гроші захищені до події» наскрізно + trust-бейджі організатора; типізований dispute + докази + статус з SLA + UA air-alert авто-резолвер; прозорий строк/сума refund; early-payout апсел (+3%); payout-latency як контр-NSM | P1 | high | M | trust-conversion; organizer retention; chargeback; early-payout revenue | INIT-CHANNELS |
| **INIT-TICKET-LIFECYCLE** | Ticket-lifecycle як єдиний модуль: іменні квитки + transfer + per-ticket escrow/refund | Неіменні квитки + безлімітний transfer = скальпінг; escrow-власність/partial-refund/refund-політика не специфіковані; double-spend transfer↔scan | Per-ticket стан-машина: іменні (owner_id) + transfer-seam (атомарно, ліміт+cooldown, guest-claim до входу); per-ticket escrow/promo під `FOR UPDATE`; refund-політика як конфіг-об'єкт (HELD→100%); власність → поточному owner; атомарність transfer↔scan; «обмін» = transfer-на-tier з доплатою | P1 | high | L | fraud/chargeback; коректність refund; double-entry; support-тікети | INIT-TICKET-UNCLAIMED, INIT-CONTRACTS |
| **INIT-QUICKCREATE** | Quick-create («Luma-mode») + KYC just-in-time + post-publish share | Обов'язкові tiers/sales-windows/KYC-онбординг; немає «ось лінк — поділись»; TTFS не визначено | Дефолт: title+дата+місто+обкладинка+«Безкоштовно/Платно»; publish <3хв; решта за «Розширеними»; KYC-банер лише при cleared-балансі + «Запросити виплату»; post-publish share-екран (URL+Telegram/WhatsApp/QR); «Дублювати подію»; ends_at обов'язковий для платних | P1 | high | M | organizer activation; TTFS; supply-side K-factor | INIT-CONTRACTS |
| **INIT-CHECKOUT** | Простий за замовчуванням checkout + all-in ціна + native wallet express | Checkout несе tier+qty+promo+gift+guest+escrow+age одночасно; «сервісний збір» окремим рядком (причина №1 відтоку); wallet за redirect-крок | Прогресивне розкриття (дефолт 1×1×«Купити»); free/RSVP → mint без екрана (bottom-sheet); all-in ЄДИНА сума (розбивка expandable); native wallet express-кнопка; детермінований post-payment + idempotent retry; organizer fee-transparency | P1 | high | L | checkout completion (mobile); дубль-charge; GMV/take-rate | INIT-CONTRACTS |
| **INIT-LIFECYCLE-TRIGGERS** | Тонкий lifecycle-шар: reminder, друзі-йдуть, review-nudge, abandoned-checkout + throttle | Retention відсутній; розрізнені notify без throttle = spam-берст = uninstall | ~5 named-тригерів поверх outbox (не CRM): reminder T-24h, друзі-йдуть (агрегація+cap), review-nudge T+год, abandoned-checkout T+1h, scarcity; один throttle-gate (quiet-hours+cap+dedup); соц-пуші → денний дайджест | P1 | high | M | D7/D30; review-conversion; checkout completion; opt-out↓ | INIT-CHANNELS, INIT-ANALYTICS |
| **INIT-COLDSTART** | Cold-start & supply-seeding playbook + non-friend соц-доказ + liquidity gate | Demand-first над порожнім Києвом → ghost-town; немає seeding/single-player value/import | Concierge-онбординг 20-30 київських організаторів на FREE/0%; organizer single-player value (квитки+QR+door+CSV); «створити з посилання/афіші» + semi-manual import; non-friend соц-доказ; liquidity launch-gate як умова public-запуску | P1 | high | M | liquidity (події/тиждень); activation організаторів; search-success | — |
| **INIT-SEO** | Органічний пошук: SSR home + JSON-LD + гео/категорійні лендінги | `page.tsx = 'use client'`; немає JSON-LD Event, `/c` `/city`; фундамент готовий | Прибрати `'use client'` з home (SSR); JSON-LD Event + ItemList; canonical `/c/[category]`, `/city/[slug]` (Київ-first) як найдешевший long-tail; паралельний трек | P1 | high | M | organic traffic; non-paid частка нових; indexed pages | — |
| **INIT-ORGANIZER-CHANNEL** | Organizer-as-channel: branded URL + embed-widget + UTM/referral | Найдешевший CAC-канал не сформульований; немає branded URL, embed, атрибуції | Branded event-URL + embeddable «Buy ticket» widget (iframe/script) + UTM/referral у воронці; мінімальний promoter MVP на escrow-seam (tracked link + split) | P2 | high | L | CAC; частка GMV з organizer-traffic | INIT-ANALYTICS |
| **INIT-RSVP-SOCIAL** | RSVP-toggle як соц-driver + приватність + базова соцмережність + anti-spam | Лічильники «друзі йдуть» без RSVP-кнопки; авто-публічність = privacy-інцидент; composer без rate-limit | RSVP-toggle як primary; дефолт-приватність за типом події + тумблер; «друг»=mutual-follow; єдиний block/mute+privacy фільтр ПЕРЕД показом; reverse-chronological feed; rate-limit+санітизація | P2 | medium | M | RSVP→conversion; retention; share of spam | INIT-CHANNELS |
| **INIT-EXPERIMENTS** | Мінімальний feature-flag + exposure seam для A/B | Немає A/B; ризиковані гіпотези вводяться як факт без вимірювання/відкату | Deterministic hash-bucketing за `user_id/anon_id` + `experiment_exposed{key,variant}`; перші експерименти: guest-wall, friends-going, ваги feed; НЕ платформа | P2 | medium | M | усі ключові (причинна вимірність) | INIT-ANALYTICS |
| **INIT-COMPLIANCE-GUARDS** | Ліміти делегованих грошових примітивів + GDPR-erasure телеметрії + report-петля | staff refund без ліміту/co-sign; transfer без undo; erasure не чистить `telemetry_events`; report без feedback | Staff refund: cap+co-sign+risk-тег, дефолт off; transfer: undo-вікно+підтвердження+cooldown; GDPR: telemetry у erasure, анонімізація guest-props; report: видима петля + типи фроду + авто-freeze | P2 | medium | S | internal-fraud loss; compliance; K-factor (safe community) | — |
| **INIT-RESALE** | Face-value P2P-resale (пост-ліквідність) з резервуванням полів зараз | Resale зник з документів; без легального ринку перепродаж піде в Telegram із шахрайством | Стан `listed` + list/unlist/buy-listed; стеля = номінал tier; комісія 5-10%; викуп через escrow-seam; waitlist як попит-сигнал; реалізацію відкласти, `resale_state` зарезервувати зараз | P2 | medium | L | GMV вторинного ринку; sell-through; частка перепродажу в системі | INIT-TICKET-LIFECYCLE |
| **INIT-LOCALE-ASO** | UA-first локалізація + локальний PSP day-1 + ASO-базис | Web частково EN; онбординг вимагає SWIFT; ASO не згадано; Stripe Atlas як PRIMARY | 100% UA UI (en лише для нових ринків); payout через IBAN/картку без обов'язкового SWIFT; локальний UA-PSP (LiqPay/monobank + native Apple/Google Pay) PRIMARY за provider-agnostic seam; Stripe — worldwide-шар; ASO-артефакт; Telegram-share first-class | P2 | medium | M | activation організаторів; D7; organic installs; UA conversion | INIT-CONTRACTS, INIT-ROADMAP-SPEC |
| **INIT-DESIGN-TOKENS** | Design-token контракт + non-color redundancy + системні стани UI | Токени не обов'язкові для 30+ поверхонь; вердикти лише кольором; empty/skeleton/error не специфіковані; M9 BLOCKED | `design-tokens.css` — єдине обов'язкове джерело (lint на raw-hex); non-color redundancy скрізь (іконка+текст+contentDescription, контраст 4.5:1); системний набір станів; one-hand ергономіка (bottom-CTA, 48dp); a11y-sweep icon-only | P2 | medium | M | retention (якість); organizer success; a11y-покриття | — |

¹ INIT-DOORSTAFF позначено P0 у частині листка **STAFF-1a** (делегований event-scoped scan — блокер входу); листки 1b/1c/1d — P1/P2.

---

## 5. Лінія відсічення MVP (Simple Launch)

**ПЕРШИЙ ПРОСТИЙ РЕЛІЗ** — free/RSVP, без реальних грошей, з `NoopProvider`. Мета: замкнути end-to-end віральну петлю з робочими каналами, безпекою й вимірюваністю.

### Входить у Simple Launch

| Крок | Що саме | Ініціативи |
|---|---|---|
| Вхід | Google one-tap + passwordless email-код у claim-контексті | INIT-BROWSE-AUTH |
| Відкриття | Browse-before-auth Афіша з пошуком + фільтр міста/дати | INIT-BROWSE-AUTH, INIT-DISCOVERY (базово) |
| Дія | Деталі + RSVP → free/RSVP-квиток з безпечним signed-QR | INIT-DOORSTAFF (QR) |
| **Ядро** | Наскрізна frictionless gift/claim-петля БЕЗ акаунта | INIT-GIFTLINK на INIT-TICKET-UNCLAIMED, з INIT-ANTIABUSE |
| Розповсюдження | Universal-links + OG-прев'ю | INIT-DEEPLINKS |
| Доставка | Робочі FCM + email канали | INIT-CHANNELS |
| Вхід на подію | Делеговане door-scan одним контролером (event-scoped, без offline/audit) | STAFF-1a (INIT-DOORSTAFF) |
| Вимірність | Tracking plan + воронка + K-factor | INIT-ANALYTICS |
| Онбординг | Не-блокуючий ≤1 екран | INIT-ONBOARDING |
| Контракти | minor-units/timezone/i18n-каркас + фікс часу | INIT-CONTRACTS |
| Платежі | Лише `trait PaymentProvider` + `NoopProvider` | INIT-ROADMAP-SPEC |

### Відкладено у Фазу 4 (real money)

ledger double-entry · reconciliation · generic webhook receiver + provider registry · admin financial-KPIs/dispute-drawer · KYC/KYB queue · anti-fraud risk-console · bulk payouts · ticket tiers з per-tier UI.

### Відкладено у пізніші фази

paid checkout/native wallet · dispute-workflow · ticket-lifecycle (іменні квитки/transfer-ліміти/partial-refund) · escrow-trust UI · quick-create paid · SEO-лендінги · lifecycle-тригери · cold-start playbook (операційно паралельно) · resale · organizer-channel · повна соцмережність.

---

## 6. Що спрощуємо / викидаємо (боротьба з роздуванням)

| Область | Замість (роздуте) | Робимо (просте) |
|---|---|---|
| **Соцмережа** | Композер з visibility, comment threads/likes, repost, media-grid, @mentions, QR-візитка, followers-екрани | friends-going + RSVP-об'єкт + share/invite/gift + аватари/verified. Це «другий Instagram» — не рухає K-factor (консенсус усіх дисциплін) |
| **Месенджинг** | Real-time DM (WebSocket + typing + presence + read-receipts) | 4с polling до PMF; вірусний канал — Telegram/WhatsApp, не власний DM |
| **Токени** | Окремі magic-link + claim-token стеки | ОДИН signed-token seam для gift/guest/staff-invite |
| **Lifecycle** | Важкий CRM / сегмент-білдер / lifecycle-платформа | ~5 named-тригерів + один throttle-gate; мінімальний preference-center |
| **Гроші** | ledger double-entry / reconciliation / financial-KPIs / dispute-drawer | Немає реальних грошей — нічого звіряти → Фаза 4 |
| **Провайдери** | Generic webhook receiver + provider registry + KYC/KYB queue + anti-fraud + bulk payout | Noop/Test + ОДИН UA-PSP; compliance-машина — до моменту виплат |
| **Tiers** | Per-tier capacity/sold/UI + sales-windows/min-max/visibility(locked) | Один price/free; повний editor після перших платних китів |
| **Feed** | Формула ранжування (recency·w1 + engagement·w2 + friends_boost) | Reverse-chronological + pull-to-refresh на старті |
| **QR** | TOTP/HOTP rotating-QR | Підписаний stable token + short-exp + серверний online anti-passback; сирий `Text(qrToken)` прибрати одразу (5-хв фікс) |
| **Escrow** | Required-checkbox на кожній покупці; «сервісний збір» окремим рядком | Пасивний disclosure; all-in одна сума |
| **Scan** | Offline Room-черга + scan_events audit | Простий online-scan (95% кейсів) → P2 |
| **Discovery** | Map-view на першому екрані; 6 сегментів + 5 фільтрів | Секція «Поруч» списком; «Популярне поблизу»+пошук+дата |
| **Door-staff** | 4 permission-чіпи (scan/view_list/refund/manage) | Лише scan (+owner-implicit); refund прибрати з дверей |
| **Аналітика орг.** | Повна воронка на видноті | 3 числа (Продано/Виручка/Залишилось), воронка в окрему вкладку |
| **Gift UI** | 3 способи подарувати на одному екрані; множинні share-кнопки | claim-link дефолт, решта за «інші способи»; один bottom-sheet |
| **Compliance/A-B/FX** | Повний risk-console/KYB-для-всіх/важка A-B/FX/RTL зараз | Inline-гейти + легкий individual-KYC + hash-bucketing flag + guardrail-правила |
| **Block/Mute** | Block+Mute як P0 | Block достатньо, лише перед public-listing; Mute викинути |
| **Не тягнути навіть у згадках** | event_sessions/multi-day, seating, waitlist, recurring, Wallet-паси, saved payment, Stripe Atlas C-corp як primary | Заморозити до density в Києві |

---

## 7. Спец-розділ: віральний контур (end-to-end)

**Ціль:** людина БЕЗ акаунта отримує лінк у месенджері → бачить цінність → забирає білет-подарунок → реєструється тут же в пару кліків → сама ділиться далі. Часткова реалізація = 0 K-factor, тому контур будується цілісно.

### Як це працює покроково

| # | Крок | Що відбувається | Ініціатива |
|---|---|---|---|
| 1 | **Створення лінка** | Дарувальник тисне «Подарувати» → `POST /api/tickets/:id/gift-link` → білет переходить у стан **`unclaimed`**, escrow утримується за дарувальником, генерується **universal https-лінк** (не голий `underground://`) | INIT-TICKET-UNCLAIMED, INIT-GIFTLINK |
| 2 | **Шеринг** | Лінк кидається у Telegram/WhatsApp; месенджер тягне **OG-прев'ю** (динамічний image 1200×630: «‹Ім'я› дарує тобі квиток на ‹Подія›», дата·місто, аватар-стек) | INIT-DEEPLINKS |
| 3 | **Відкриття без апки** | Отримувач без акаунта відкриває `/g/[token]` → **публічна landing** показує де/коли/tier, QR **розмито**, хто подарував — **цінність ДО логіну** | INIT-GIFTLINK, INIT-BROWSE-AUTH |
| 4 | **Забір** | Одна кнопка «Забрати» → **Google one-tap АБО passwordless email-код НА ТІЙ САМІЙ сторінці** → атомарна прив'язка `ticket.owner_id`, перший авторизований claim → `active` | INIT-GIFTLINK, INIT-BROWSE-AUTH |
| 5 | **Реєстрація** | Акаунт створюється ПІСЛЯ забору (≤2 кліки); identity-merge: guest-email → при першому Google-логіні з тим самим email авто-злиття unclaimed-білетів | INIT-BROWSE-AUTH |
| 6 | **Re-share** | На екрані успіху — **обов'язковий вихідний share/gift-CTA** («Візьми друга — подаруй другий») → новий вузол одразу стає джерелом петлі | INIT-GIFTLINK |
| 7 | **Доставка** | Кожен крок (запрошення, «переглянуто», «забрано», нагадування) доставляється через **робочі FCM+email** | INIT-CHANNELS |

### Безпека контуру (умова готовності, не окрема задача)

- **single-use + short-TTL** підписаний claim-token (окремий секрет від QR-токена);
- **rate-limit** на `/g /t/:token` + **device/email-cap** на free-claim;
- **magic-code** — підтвердження власності (код у листі, не лише лінк) проти крадіжки по відкритому першим лінку;
- позначка «квиток забрано з іншого пристрою»; **auto-revert** до дарувальника по TTL;
- SEC-вектори у SDD: token brute, replay, double-claim, claim-after-sold-out.
- **Фічу не вмикати без контуру** (INIT-ANTIABUSE).

### Ключові інваріанти

- Email/@username — **другорядний** варіант за «інші способи»; дефолт — claimable link (щоб «хто перший забрав» працювало).
- Комісія **НЕ береться** з подарованих/guest/free квитків — петля лишається абсолютно frictionless.
- **gift-claim ≠ guest checkout:** два окремі flow. gift-claim (отримувач-не-юзер) — центральна P0-петля; guest checkout (покупець, платний) — P1.

---

## 8. Послідовність фаз

> Кожна ініціатива зшивається з детальними тех-завданнями (SDD/AC/SEC/Test-Matrix) через `ID` у **окремому файлі тех-завдань**.

| Фаза | Мета | Ініціативи |
|---|---|---|
| **Фаза 0 — Фундамент та єдине джерело істини** (тижні 1-2) | Зняти планувальні конфлікти й закласти дешеві незворотні контракти ПЕРЕД фіче-кодом; вирішити всі spec-рішення (auth-виняток, product-first, minor-units, timezone) | INIT-ROADMAP-SPEC, INIT-CONTRACTS |
| **Фаза 1 — Frictionless віральний контур** (перший цінний реліз = Simple Launch) | Замкнути end-to-end петлю «лінк без акаунта → подарунок → реєстрація тут же → re-share» з робочими каналами, безпекою й вимірюваністю | INIT-TICKET-UNCLAIMED, INIT-CHANNELS, INIT-DEEPLINKS, INIT-ANTIABUSE, INIT-GIFTLINK, INIT-BROWSE-AUTH, INIT-ONBOARDING, INIT-ANALYTICS, INIT-DOORSTAFF (STAFF-1a) |
| **Фаза 2 — Простота відкриття, довіра й supply-активація** | Афіша — двигун відкриття, escrow — видима обіцянка, організатор активований <3хв; retention-петлі; готувати ліквідність до public | INIT-DISCOVERY, INIT-TRUST-ESCROW, INIT-QUICKCREATE, INIT-LIFECYCLE-TRIGGERS, INIT-COLDSTART, INIT-SEO |
| **Фаза 3 — Платежі, ticket-lifecycle, зростання** | Реальні гроші за provider-agnostic seam з простим checkout; exchange/transfer/refund у коректний домен; ріст organizer-каналом + базова соцмережність | INIT-CHECKOUT, INIT-TICKET-LIFECYCLE, INIT-ORGANIZER-CHANNEL, INIT-RSVP-SOCIAL, INIT-LOCALE-ASO, INIT-EXPERIMENTS, INIT-DESIGN-TOKENS |
| **Фаза 4 — Real-money back-office, вторинний ринок, compliance** | Грошова коректність промислового рівня (ledger, reconciliation, dispute-workflow, KYC/KYB just-in-time); face-value resale після ліквідності | INIT-COMPLIANCE-GUARDS, INIT-RESALE |

### Розв'язані конфлікти (закріпити у spec)

| Конфлікт | Рішення |
|---|---|
| Money-first vs product-first | **product-first, payment last** у `spec.md`; M1 → Фаза 4 |
| Google-only vs frictionless claim | Google для повних акаунтів + passwordless email-код ЯК ВИНЯТОК у claim/guest; identity-merge. Апрув засновника + spec |
| Пріоритет guest/gift-claim | Підняти до **P0** на обидва клієнти ПЕРЕД auth-wall |
| All-in vs «сервісний збір» | All-in одна сума; розбивка expandable |
| Escrow trust vs supply-тертя | Видимий покупцю + early-payout (+3%) + payout-передбачуваність |
| Anti-scalping vs giftability | Іменні квитки + transfer-seam з лімітом + guest-claim до входу |
| Frictionless vs safety | Anti-abuse контур зберігає пару кліків для чесного юзера |
| Rotating-QR ⊥ offline | Signed stable token + серверний online anti-passback; TOTP відкласти |
| Ідемпотентність входу | Ключ `qr_token/ticket_id`, first-write-wins (SEC-виправлення, не опція) |
| minor-units vs NUMERIC | minor-units i64 — єдиний канон, фіксувати зараз |
| UA-PSP vs Stripe primary | Provider-agnostic seam: UA-PSP day-1, Stripe — worldwide-шар |
| SEO vs месенджер-шеринг | Обидва, але gift-claim+OG ВПЕРЕД SEO; SEO — паралельний дешевий трек |
| «Для тебе» в Афіші й Feed | Афіша=події, Feed→«Активність»; «Для тебе» лише в Афіші |
| Guest checkout vs gift-claim | Два окремі flow; gift-claim — P0, guest checkout — P1 |

---

## 9. Метрики та як міряти успіх

### 9.1. North Star і контр-метрики

| Тип | Метрика | Ціль/SLA |
|---|---|---|
| North Star | Квитки (paid+free/RSVP) на активного користувача / 28 днів | зростання cohort-over-cohort |
| Вхідна росту | **K-factor** = invites_accepted / active_users | **≥ 0.5** |
| Supply контр-NSM | payout-latency (`ends_at → PAID_OUT`) | SLA + алерт на застряглі кошти |
| Activation | `user_activated` = перший з {ticket_claimed \| rsvp_set \| follow≥3} у 24-48h | — |
| TTFW / TTFS | time-to-first-wow / time-to-first-sale | TTFS публікація <3 хв |

### 9.2. Пороги готовності релізу (pruning-тест для P0)

| Метрика | Поріг |
|---|---|
| share → claim | ≥ 30% |
| guest → signup | ≥ 40% |
| K-factor | ≥ 0.5 |

### 9.3. Tracking Plan v1 (~20 канонічних подій)

`event_view{event_id,source}` · `tier_select` · `checkout_started{tier_id,qty,amount_minor,is_guest}` · `checkout_paid` · `checkout_failed{reason}` · `rsvp{state}` · `gift_sent` ↔ `gift_claimed` · `invite_sent{channel}` ↔ `invite_accepted` · `share_clicked{surface}` · `scan_result{verdict}` · `onboarding_step{step,completed}` · `app_open/session`.

- 3-6 props на подію, **один канонічний point-of-emit**, розширює `telemetry::log` (нуль нових екранів).
- Парна атрибуція `share_token/referrer_id` + `attributed_signup` — обчислення K-factor.
- Server-side purchase-воронка: `view → detail_open → checkout_started → paid → attended` з drop% між кроками.

### 9.4. Дашборд

`GET /api/admin/telemetry/funnel` (signup→onboarding→first_save→first_ticket→share) + timeseries + сегментація по `source/channel`; north-star дашборд активації/ретеншну (MAU/D7/D30) дзеркально до фінансового timeseries.

### 9.5. Метрики за фазами

| Фаза | Головні метрики |
|---|---|
| Фаза 0 | delivery predictability; reconciliation-accuracy; no-show-rate (правильний час) |
| Фаза 1 | **K-factor**; share→claim; guest→signup; TTFW; delivery success rate; fraud-loss; double-entry rate |
| Фаза 2 | card→detail CTR; D1/D7/D30; trust-conversion; TTFS; liquidity (події/тиждень); organic traffic |
| Фаза 3 | checkout completion (mobile); GMV/take-rate; chargeback; CAC; RSVP→conversion |
| Фаза 4 | payout-correctness; internal-fraud loss; GMV вторинного ринку; compliance-готовність |