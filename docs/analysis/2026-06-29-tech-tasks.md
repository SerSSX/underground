# Underground — Точні тех-завдання (ТЗ) з аудиту

> Згенеровано 2026-06-29 з майстер-роадмапу (аудит PM/Marketing/Design). Кожне ТЗ — у стилі `docs/specks/SDD` (AC + SEC + Test Matrix), готове до декомпозиції в `docs/backlog/`.
> North Star: Квитки (paid + free/RSVP) на активного користувача за 28 днів — метрика, що поєднує активацію попиту, віральність і GMV, не караючи free-first старт. Supply-side контр-метрика: payout-latency (ends_at→PAID_OUT) з SLA. Вхідна метрика росту: K-factor (share/gift-лінк → claim → signup → re-share) ≥ 0.5.

## Зміст

- **SDD-ROADMAP-ORDER** — Product-first, payment-last як канонічний порядок релізів у docs/spec.md (єдине джерело істини) (`S`)
- **SDD-CONTRACTS-1** — Незворотні контракти: minor-units гроші, event_timezone, i18n-каркас + фікс рендеру часу (`S`)
- **SDD-020** — Ticket unclaimed / pending_claim + gift на незареєстрований email (виральний seam) (`M`)
- **SDD-018** — Frictionless gift/claim-петля на єдиному signed-token seam (claimable gift-link + публічна OG-landing + inline auth + вихідний share-CTA) (`L`)
- **SDD-INIT-DEEPLINKS** — Верифіковані universal-links + OG/Twitter-прев'ю + guest gift-claim для всіх share-об'єктів (`M`)
- **SDD-CHANNELS-1** — Робочі lifecycle-канали: FCM push (HTTP v1) + transactional email за єдиним seam (`M`)
- **SDD-ANTIABUSE** — Anti-abuse контур для guest/gift-claim (single-use short-TTL token + rate-limit + device/email-cap + magic-code) (`M`)
- **SDD-BROWSE-AUTH-001** — Browse-before-auth + passwordless email-код у claim/guest-контексті (`M`)
- **SDD-ONB-1** — Не-блокуючий онбординг ≤1 екран + contextual permission priming + lazy 18+ age-gate (`S`)
- **SDD-020** — Tracking Plan v1 + purchase-воронка + K-factor атрибуція + North-Star дашборд (`M`)
- **SDD-QUICKCREATE** — Quick-create події (Luma-mode) + KYC just-in-time + post-publish share-екран (`M`)
- **SDD-CHECKOUT-1** — Простий за замовчуванням checkout + all-in ціна + native-wallet express (provider-agnostic) (`L`)
- **SDD-DISCOVERY-1** — Time-aware персональна Афіша: сектори замість плоского фільтр-екрана (`M`)
- **SDD-018-TRUST-ESCROW** — Escrow-as-trust видимий + типізований dispute/refund workflow + early-payout клапан (`M`)
- **SDD-018** — Ticket-lifecycle як єдиний домен: іменні квитки + системний transfer/gift + per-ticket escrow/refund (`L`)
- **SDD-STAFF-1a** — Делеговане door-scan (STAFF-1a): event_staff + require_perm + event-scoped validate + інвайт-лінк «Контролер» + безпечний signed QR з серверним anti-passback і повними вердиктами (`M`)
- **SDD-018** — Органічний пошук: SSR-афіша + JSON-LD + гео/категорійні лендінги (INIT-SEO) (`M`)
- **SDD-LIFECYCLE-TRIGGERS** — Тонкий lifecycle-шар: 5 named-тригерів + єдиний throttle-gate поверх наявного outbox (`M`)
- **SDD-COLDSTART** — Cold-start & supply-seeding: import подій з посилання, non-friend соц-доказ, liquidity launch-gate (`M`)

---

## SDD-ROADMAP-ORDER — Product-first, payment-last як канонічний порядок релізів у docs/spec.md (єдине джерело істини)

**Мета:** Усунути суперечність двох планів (improvement-plan M1='Real money seam' першим vs product-changes-v2 Фаза 4='реальний PSP' останнім), зафіксувавши у docs/spec.md ОДНЕ авторитетне правило порядку: продукт-ядро (free/RSVP + gifting + escrow-lifecycle на NoopProvider) виходить ПЕРШИМ, реальна платіжка/compliance/ledger-transport — у Фазі 4 після PMF. Це розблоковує заведення TASK-ів без planning-конфлікту й підпорядковує визначення P0 north-star-метриці (tickets/active-user за 28д, K-factor≥0.5), а не архітектурній важливості.

**Поверхні:** Docs (docs/spec.md — нормативний), Docs (docs/analysis/2026-06-29-improvement-plan.md — erratum), Docs (CLAUDE.md — вказівник)  ·  **Effort:** S  ·  **Залежності:** Немає код-залежностей. Розблоковує (є передумовою для): TASK-PROV-1 (trait+Noop/Test+registry, Фаза 0), TASK-LEDGER-1 (Фаза 0), TASK-TIERS-1/CHECKOUT-1/AUTH-1/... (Фаза 1), TASK-PROV-LS-1 та інші PSP (Фаза 4)., Джерела-узгодження (read-only): docs/analysis/2026-06-29-product-changes-v2.md §6 (канонічні фази), docs/analysis/2026-06-29-improvement-plan.md (M1, що перепозначається), docs/rules.md §0/§2/§3/§12., Governance: rules.md §2 (зміна поведінки/порядку спершу в spec) — ця задача і є цей крок.

**У scope:**
- Додати у docs/spec.md новий нормативний розділ (напр. §9 'Release ordering & phasing — product-first, payment-last') з 5-фазною послідовністю (Фаза 0..4), що дослівно узгоджена з docs/analysis/2026-06-29-product-changes-v2.md §6
- Зафіксувати у spec.md переоприділення P0: 'P0 = блокує ПЕРШИЙ цінний реліз (activation/virality), НЕ архітектурна важливість'
- Зафіксувати у spec.md інваріант: ticketing+escrow домен provider-neutral; активний провайдер на старті = NoopProvider/TestProvider; реальний PSP лише у Фазі 4 і не торкається коду Фаз 0-3
- Явно re-tag improvement-plan M1 'Real money seam' → Фаза 4 / post-beta: додати у improvement-plan.md шапку-erratum з посиланням 'superseded by spec.md §9', БЕЗ видалення історичного тексту
- У Фазі 0 spec.md залишити рівно trait PaymentProvider + NoopProvider(+TestProvider) + registry; ledger-transport (реальні перекази) — Фаза 4
- Оновити CLAUDE.md рядок про монетизацію: додати вказівку, що порядок релізів регулюється spec.md §9 (product-first)
- Оновити north-star і KPI-визначення у spec.md §2 як мірило пріоритезації (tickets/active-user 28д; K-factor≥0.5; payout-latency SLA)
- Тонка звірка узгодженості: жоден існуючий BR (BR-001..BR-017) не має суперечити provider-neutral формулюванню (замінити хардкод 'Lemon Squeezy' на 'активний PaymentProvider' у BR-004/BR-005 текстово, зі збереженням поведінки)

**Поза scope:**
- Будь-яка зміна коду в api/ (trait/Noop/registry реалізуються окремим TASK-PROV-1 — тут лише фіксуємо ПОРЯДОК у spec)
- Реалізація ledger_entries, LemonSqueezyProvider, Stripe/LiqPay — окремі TASK-и Фаз 0/4
- Створення самих TASK-###/SDD-### для фіч Фаз 0-4 (ця задача лише розблоковує їх заведення, не заводить)
- Видалення історичного тексту improvement-plan.md M1 (тільки erratum-надпис, щоб зберегти audit-trail §12)
- Зміна фактичної escrow-стейт-машини або BR-семантики (лише текстова provider-neutral переформулювання)
- UI/Android/Web зміни

**Модель даних:**
- Немає змін схеми БД / міграцій — задача суто документна (PM). Жодних migrations.
- Фіксується (як майбутній інваріант у spec, не реалізується тут): активний провайдер резолвиться за payment_settings.provider ∈ {noop,test,lemonsqueezy,...}; гроші лише через api/src/escrow.rs seam (BR-006 незмінний).
- Фіксується у spec як Фаза-0/Фаза-4 контракт (реалізація поза цією задачею): таблиця ledger_entries (immutable, append-only) — транспорт реальних грошей вмикається у Фазі 4.

**API:**
- Немає нових/змінених endpoint-ів у цій задачі (документна зміна).
- Спец фіксує МАЙБУТНІ контракти як нормативні (реалізуються окремими TASK-ами): checkout приймає {tier_id, quantity} → provider_action; generic webhook POST /api/webhooks/:provider (Фаза 4); GET /api/admin/payment-providers для динамічних полів провайдера (Фаза 4). Тут — лише запис у spec, без коду.

**UI:**
- Немає UI-екранів у цій задачі.
- Спец фіксує принцип для майбутніх екранів (нормативно): кожен ключовий екран (Афіша, create-event, checkout) стартує з ОДНОГО рішення, решта — за 'Розширеними налаштуваннями'; цінність до auth-wall (анонім бачить подію/подарунок до логіну).

**Acceptance Criteria:**
- AC-1: docs/spec.md містить новий нормативний розділ 'Release ordering & phasing' з 5 фазами (0..4), де Фаза 0=seam(PaymentProvider+Noop/Test+registry, без поведінкових змін), Фази 1-3=продукт/операції/соц на Noop/Test, Фаза 4=реальний PSP; текст фаз узгоджений 1:1 із product-changes-v2 §6.
- AC-2: spec.md явно містить речення-визначення 'P0 = блокує ПЕРШИЙ цінний реліз (activation/virality), а не архітектурна важливість' і воно позначене як нормативне.
- AC-3: spec.md містить інваріант 'ticketing+escrow provider-neutral; активний провайдер на старті = Noop/Test; реальний PSP лише Фаза 4 і не змінює код Фаз 0-3'.
- AC-4: docs/analysis/2026-06-29-improvement-plan.md має видиму erratum-шапку біля розділу M1, що переносить 'Real money seam' у Фазу 4/post-beta з посиланням 'superseded by docs/spec.md §<n>'; історичний текст M1 збережено.
- AC-5: жоден залишений документ (spec.md, product-changes-v2.md, improvement-plan.md, CLAUDE.md) не містить активної (не-erratum) вимоги ставити реальні гроші/PSP першим релізом — money-first усунено як діюча директива.
- AC-6: BR-004 і BR-005 у spec.md переформульовано provider-neutral (термін 'активний PaymentProvider' замість хардкоду 'Lemon Squeezy') без зміни поведінки/семантики ескроу.
- AC-7: CLAUDE.md блок про монетизацію посилається на spec.md §<n> як на джерело порядку релізів (product-first).
- AC-8: Порядок пріоритетів у spec явно підпорядкований north-star (tickets/active-user 28д) + K-factor≥0.5 + payout-latency SLA; ці метрики названі критерієм 'що є P0'.
- AC-9: Зміна поведінки відсутня → жоден автотест не червоніє; ./scripts/ci.sh лишається зеленим (документна задача, §8.6).
- AC-10: Порядок §0-precedence дотримано: spec.md залишається нижче System/User/rules.md; ця задача не суперечить rules.md і явно посилається на §0/§2.

**Security-вектори (SEC):**
- SEC-1: На старті (Noop/Test) жоден код-шлях НЕ рухає реальних грошей — spec фіксує це як інваріант Фази 0; live-mode (Фаза 4) вимагає всіх creds провайдера, інакше charge/payout заборонено.
- SEC-2: Documentation-integrity/anti-drift: після merge не існує двох суперечливих активних директив порядку (money-first vs product-first) — інакше agent може реалізувати небезпечний money-first шлях передчасно; verify grep-ом.
- SEC-3: BR-006 (уся грошова логіка лише через escrow.rs seam) і immutable append-only ledger залишаються незмінними у переформулюванні — provider-neutral текст не послаблює money-seam-контроль.
- SEC-4: erratum не видаляє audit-trail (rules §12) — зберігається історія рішення, що є вимогою комплаєнсу/трасування.
- SEC-5: Frictionless guest/gift-петлі, які spec робить P0, у нормативному тексті супроводжуються умовою anti-abuse (single-use short-TTL signed token + rate-limit + device/email-cap) як умова готовності, а не окрема задача.

**Test Matrix:**
- [POS] grep spec.md → присутній розділ 'Release ordering' з усіма 5 фазами у правильному порядку (Noop→...→реальний PSP).
- [POS] grep spec.md → присутнє переоприділення P0 (рядок про 'перший цінний реліз').
- [POS] grep improvement-plan.md → erratum-шапка з 'superseded by spec.md' біля M1 присутня; оригінальний текст M1 усе ще присутній.
- [POS] grep CLAUDE.md → посилання на spec.md §<n> для порядку релізів присутнє.
- [NEG] grep по всіх doc → немає активної директиви 'реальні гроші/PSP першим' поза erratum-контекстом (0 збігів money-first як діючої вимоги).
- [NEG] Фаза 0 у spec НЕ містить реального ledger-transport/LemonSqueezy (лише trait+Noop/Test+registry) — інакше fail.
- [NEG] BR-004/BR-005 не містять хардкоду 'Lemon Squeezy' як єдиного провайдера у нормативному тексті.
- [SEC] Немає двох конфліктних активних послідовностей (money-first vs product-first) одночасно активними — verify-grep.
- [SEC] BR-006 + immutable ledger формулювання збережені після edit (diff показує лише provider-neutral переформулювання, не послаблення).
- [SEC] Frictionless-P0 пункти у spec несуть anti-abuse умову готовності (short-TTL single-use token + rate-limit + device/email-cap).
- [POS] ./scripts/ci.sh зелений після змін (жодного коду не торкнуто, поведінка незмінна).

---

## SDD-CONTRACTS-1 — Незворотні контракти: minor-units гроші, event_timezone, i18n-каркас + фікс рендеру часу

**Мета:** Зафіксувати три дешеві-зараз / дорогі-потім незворотні контракти ДО того, як продукт стане живим, щоб уникнути breaking-міграцій на проді: (1) канонічні гроші в minor-units i64 на межі API/ledger з ЄДИНОЮ точкою NUMERIC↔minor конвертації в репо-шарі та per-currency decimal-exponent; (2) колонка events.event_timezone (IANA) з дефолтом із міста, і показ часу події в її поясі з підписом «за місцевим»; (3) i18n-каркас Android (values/strings.xml + values-en) під нові екрани та guardrail на start/end у rules. Головна користь для north-star: правильний локальний час знижує no-show-rate, minor-units дає reconciliation-accuracy й скорочує time-to-worldwide. Рішення тримаємо ПРОСТИМ: не переписуємо існуючі 306 кирилиць-літералів масово (окремий борг), а фіксуємо каркас + guardrail; гроші конвертуємо в одному seam, домен далі рахує в Decimal.

**Поверхні:** API (Rust/Axum): api/src/money.rs (новий), api/src/currency.rs, api/src/cities.rs, api/src/events.rs, api/migrations/0033_event_timezone.sql, Android (Kotlin/Compose): app/build.gradle.kts, util/Format.kt, res/values-en/strings.xml, data/Event.kt (event_timezone), Web (Next.js): web/lib/i18n.tsx (fmtDateL), web/lib/api.ts (Event type += event_timezone), Docs: docs/spec.md, docs/rules.md  ·  **Effort:** S  ·  **Залежності:** Узгодження контракту amount_minor:i64 з TASK-LEDGER-1 (ledger_entries.amount_minor) та TASK-PROV-1 (trait PaymentProvider DTO) — ця задача ФІКСУЄ тип, вони його СПОЖИВАЮТЬ, chrono-tz крейт (валідація + резолв IANA) — додати в api/Cargo.toml, Android: androidx desugar_jdk_libs (coreLibraryDesugaring) — додати в build.gradle.kts, Наявний gazetteer (migration 0015 cities) — джерело city→country→tz маппінгу, Наявний web i18n-каркас (web/lib/i18n.tsx uk/en) — розширюється, не створюється з нуля, Процес (rules §3/§12): TASK-CONTRACTS-1 backlog-картка INTAKE→READY + цей SDD перед кодом; ./scripts/ci.sh зелений перед done

**У scope:**
- CONTRACT-MONEY: увести канон grошей amount_minor:i64 + currency на ВСІХ нових/змінюваних wire-DTO платіжного домену (ChargeReq/ChargeResult/PayoutReq/RefundReq/WebhookEvent з docs аналізу §2.2) і на ledger_entries.amount_minor; конвертацію NUMERIC(12,2)↔minor інкапсулювати в ЄДИНОМУ модулі api/src/money.rs (to_minor(Decimal, currency)->i64, from_minor(i64, currency)->Decimal), що читає decimal-exponent per-currency
- MONEY-EXPONENT: додати exponent(code:&str)->u32 у api/src/currency.rs (JPY/KRW=0, більшість=2) з тестом на кожну gazetteer-валюту; money.rs масштабує через 10^exponent
- CONTRACT-TZ: міграція 0033_event_timezone.sql — ALTER TABLE events ADD COLUMN event_timezone TEXT (IANA, напр. 'Europe/Kyiv'); backfill із міста через маппінг country/city→tz; NOT NULL DEFAULT 'Europe/Kyiv' після backfill
- TZ-RESOLVE: api/src/cities.rs — додати tz у gazetteer-резолвінг (cities.timezone TEXT колонка + значення для наявних міст) і повертати у Resolved; events::create/update проставляє event_timezone із resolve(city), дозволяючи явний override у CreateEventReq.event_timezone: Option<String> (валідація IANA)
- CONTRACT-TZ-DTO: EventDto += event_timezone: String; CreateEventReq/UpdateEventReq += event_timezone: Option<String>
- FIX-RENDER-ANDROID: увімкнути coreLibraryDesugaring у android/app/build.gradle.kts; Format.eventDateTime/eventDate/isPast/isoToMillis переписати через java.time (Instant.parse + ZoneId.of(event_timezone)) замість iso.substring(11,16); показ '9 чер · 19:00 за місцевим' коли пояс події ≠ пояс пристрою
- FIX-RENDER-WEB: web/lib/i18n.tsx fmtDateL приймає tz і форматує через Intl.DateTimeFormat({timeZone}) замість iso.slice(11,16); підпис 'за місцевим'/'local' коли tz події ≠ tz браузера
- I18N-ANDROID-SCAFFOLD: створити android/app/src/main/res/values-en/strings.xml (паралель до наявного values/strings.xml); НОВІ рядки цієї задачі — лише через stringResource; існуючі 286+ хардкод-кирилиць лишаються борг-таском (не масова заміна)
- RULES-GUARDRAIL: у docs/rules.md додати правило (§5-адитивно): (a) заборона iso.substring/slice для рендеру часу — тільки tz-aware формат; (b) start/end layout guardrail — старт екранів з одного рішення; (c) нові user-facing рядки Android — тільки через stringResource, не літерал
- SPEC-UPDATE: у docs/spec.md зафіксувати як істину: гроші=minor-units i64 на API/ledger; кожна подія має event_timezone; час показується в поясі події

**Поза scope:**
- Реальний PaymentProvider (Stripe/LiqPay/LS) — окрема Фаза 4 (TASK-PROV-LS-1); тут лише КОНТРАКТ minor-units, не інтеграція
- Масова заміна всіх 306 наявних хардкод-кирилиць у Android на stringResource — окремий debt-таск (TASK-I18N-DEBT); тут лише каркас values-en + правило для нових рядків
- Повний ledger_entries функціонал (double-entry запис при mint/sweep/payout) — TASK-LEDGER-1; тут лише узгодження, що amount у ledger = minor i64
- event_sessions / багатоденні події — P2, поза межами
- Зміна БД-типу events.price з NUMERIC на BIGINT — НЕ робимо; domain лишається Decimal, minor конвертується лише на wire/ledger межі (простіше, менший blast-radius)
- Vault/wallet Apple/Google Pay, checkout UI зміни — інші таски (TASK-CHECKOUT-1)
- DM/feed/discovery — інші ініціативи

**Модель даних:**
- migration 0033_event_timezone.sql: ALTER TABLE events ADD COLUMN event_timezone TEXT; backfill UPDATE events SET event_timezone = (маппінг з cities/country, дефолт 'Europe/Kyiv'); ALTER COLUMN event_timezone SET NOT NULL SET DEFAULT 'Europe/Kyiv'
- migration 0033 (той самий файл): ALTER TABLE cities ADD COLUMN timezone TEXT; UPDATE cities SET timezone=... для кожного наявного міста (Київ→Europe/Kyiv, Warsaw→Europe/Warsaw, London→Europe/London, New York→America/New_York, Tokyo→Asia/Tokyo, ... по всіх 70+ рядках 0015)
- БЕЗ зміни events.price/tickets_total типів — лишаються NUMERIC(12,2)/INT; minor-units живе лише на wire/ledger
- ledger_entries.amount_minor BIGINT (узгодження контракту з TASK-LEDGER-1; тут не створюється таблиця, лише фіксується тип у spec)
- Валідація на write: event_timezone MUST бути валідним IANA id (перевірка через chrono-tz Tz::from_str); інакше 400

**API:**
- POST /api/events — CreateEventReq += event_timezone: Option<String> (IANA); якщо null → резолв із city; невалідний IANA → 400 invalid_timezone
- PATCH /api/events/:id — UpdateEventReq += event_timezone: Option<String> (owner-only, валідація IANA)
- GET /api/events, GET /api/events/:id — EventDto += event_timezone: String (завжди присутнє після backfill)
- GET /api/cities?q= — City += timezone: String (для клієнтського прев'ю поясу при виборі міста)
- Внутрішній контракт (не HTTP): money::to_minor(Decimal, &str)->Result<i64>, money::from_minor(i64,&str)->Decimal; усі wire-DTO платіжного seam серіалізують amount_minor:i64 (не Decimal)
- Нормалізована WebhookEvent та Charge/Payout/Refund DTO використовують amount_minor:i64 + currency:String (узгоджено з §2.2 аналізу)

**UI:**
- Android EventCard / EventDetailScreen: час події через Format.eventDateTime(iso, event_timezone); суфікс 'за місцевим' (stringResource R.string.time_local) коли ZoneId події ≠ ZoneId.systemDefault()
- Android CreateEventScreen: read-only рядок 'Пояс події: {event_timezone}' (авто з міста), під 'Розширені налаштування' — опційний override-picker поясу (простота за замовчуванням)
- Web EventCard / EventDetailClient / me/tickets: fmtDateL(iso, lang, event_timezone) з підписом local/за місцевим
- Web create-event форма: авто-показ поясу з міста + advanced override
- Жодних нових повноекранних сутностей — лише коректний рендер існуючого часу + один суфікс

**Acceptance Criteria:**
- AC-1: Подія в Києві (event_timezone='Europe/Kyiv', starts_at=17:00Z) на Android і Web показує '19:00' (літо, +3), а не '17:00'; на пристрої з іншим поясом додається суфікс 'за місцевим'/'local'
- AC-2: money::to_minor(Decimal('650.00'),'UAH')==65000; from_minor(65000,'UAH')==Decimal('650.00'); to_minor(Decimal('650'),'JPY')==650 (exponent 0); round-trip ідемпотентний для всіх gazetteer-валют
- AC-3: currency::exponent повертає 0 для JPY/KRW, 2 для UAH/USD/EUR/GBP/PLN та решти; тест перебирає кожну валюту з for_country
- AC-4: Міграція 0033 backfill проставляє event_timezone для 100% наявних events (жоден NULL); нова подія без явного поясу успадковує пояс міста; невідоме місто → 'Europe/Kyiv'
- AC-5: POST/PATCH /api/events з невалідним event_timezone (напр. 'Mars/Phobos') → 400 invalid_timezone; валідний IANA приймається
- AC-6: EventDto завжди містить event_timezone: String (non-null) у JSON відповіді; клієнти десеріалізують без падіння
- AC-7: coreLibraryDesugaring увімкнено; Format більше не містить iso.substring(11,16)/iso.slice(11,16); grep по кодовій базі не знаходить substring/slice-рендеру часу
- AC-8: values-en/strings.xml існує і містить усі нові ключі цієї задачі (time_local, event_timezone_label тощо); нові рядки в UI-коді — через stringResource, не літерал
- AC-9: docs/spec.md зафіксовано: гроші=minor-units i64 на API/ledger, кожна подія має event_timezone, час показується в поясі події; docs/rules.md містить guardrail проти substring-рендеру часу та вимогу stringResource для нових Android-рядків
- AC-10: wire-DTO платіжного seam (Charge/Payout/Refund/Webhook) серіалізують amount_minor:i64+currency, НЕ Decimal; domain-код далі рахує в Decimal, конверсія лише в money.rs на межі

**Security-вектори (SEC):**
- SEC-1: IANA-timezone injection — event_timezone валідується whitelist-парсером (chrono_tz Tz::from_str), не потрапляє сирим у форматування/SQL; невалідне → 400, не 500 і не збережене
- SEC-2: Money-precision/overflow — to_minor перевіряє, що Decimal має ≤exponent знаків після коми (інакше помилка, не тихе округлення), і що результат вміщується в i64 (checked множення); запобігає втраті грошей та overflow-атаці на ledger
- SEC-3: Negative/NaN money — to_minor відхиляє від'ємні суми там, де контракт вимагає ≥0 (charge/payout amount), уніфіковано на межі, не в кожному викликачі
- SEC-4: Timezone-DoS/ambiguity — рендер не кидає виняток на межах DST (java.time/Intl обробляють коректно); тест на 02:30 у ніч переведення годинників не падає
- SEC-5: Backfill-integrity — міграція 0033 не лишає жодного NULL event_timezone і не змінює наявних starts_at/ends_at (UTC-інстанти незмінні; змінюється лише presentation), щоб не зсунути вже продані квитки
- SEC-6: i18n-injection — жоден stringResource не форматується з неекранованим user-input у HTML/лог; нові en-рядки не містять format-string вразливостей

**Test Matrix:**
- [POS] to_minor/from_minor round-trip для UAH/USD/EUR/JPY/KRW/PLN (650.00→65000, 650→650 для JPY)
- [POS] exponent(): UAH=2, JPY=0, KRW=0, USD=2 — і тест-перебір усіх gazetteer-валют
- [POS] Київська подія 17:00Z показує 19:00 (Android java.time + Web Intl) улітку, 18:00 взимку (+2)
- [POS] backfill 0033: всі наявні events отримують не-NULL пояс; Warsaw→Europe/Warsaw, Tokyo→Asia/Tokyo
- [POS] нова подія з city='Львів' без явного поясу → event_timezone='Europe/Kyiv'
- [POS] явний валідний override event_timezone='Europe/Warsaw' зберігається і повертається у DTO
- [POS] пристрій у America/New_York дивиться київську подію → показ київського часу + суфікс 'за місцевим'
- [NEG] POST /api/events event_timezone='Not/AZone' → 400 invalid_timezone (не 500)
- [NEG] to_minor(Decimal('650.001'),'UAH') (3 знаки при exponent 2) → помилка precision, не тихе округлення
- [NEG] from_minor на i64::MAX не панікує; to_minor на сумі > i64 діапазону → checked-помилка
- [NEG] EventDto зі старого клієнта без поля event_timezone на write → резолв із city (не падіння)
- [NEG] DST-межа: рендер starts_at, що потрапляє в неіснуючу локальну годину, не кидає виняток
- [SEC-1] event_timezone з SQL-мета-символами відхилено валідатором до збереження
- [SEC-2] to_minor overflow (велика Decimal) → Err, ledger не отримує зіпсоване значення
- [SEC-3] to_minor(-100,'UAH') для charge-контексту → Err (від'ємна сума)
- [SEC-5] міграція idempotent-safe: повторний прогін не змінює вже проставлені пояси й не чіпає starts_at
- [SEC-4] форматування 02:30 у ніч переводу годинника (обидві платформи) не падає

---

## SDD-020 — Ticket unclaimed / pending_claim + gift на незареєстрований email (виральний seam)

**Мета:** Зняти технічний блокер віральної gift-петлі: дозволити подарувати/передати білет на невідомий email або через share-лінк людині БЕЗ акаунта. Ввести стан tickets.status='unclaimed' з одноразовим підписаним claim_token; transfer/gift на незареєстрований email створює білет у стані unclaimed замість падіння з Validation. Дарувальник бачить статус доставки (Надіслано/Переглянуто/Забрано) і може нагадати або відкликати білет поки його не забрали. Отримувач за посиланням бачить білет ДО логіну і забирає його в пару кліків (frictionless signup ТУТ ЖЕ). Кожен крок петлі інструментований (gift_sent→gift_viewed→gift_claimed→signup) для K-factor. Зміна поведінки фіксується у docs/spec.md ПЕРЕД кодом (rules §0/§2). Гроші НЕ рухаються — це чиста ticketing-зміна поверх provider-neutral escrow-seam.

**Поверхні:** API (api/src/tickets.rs, новий api/src/gift.rs, routes.rs, email.rs, telemetry.rs), Android (ETicketScreen gift-статус, MyTicketsScreen секція 'Подаровані', deep-link underground://gift/:token → claim-екран з Google Sign-In), Web (публічна сторінка /gift/[token] — превʼю ДО логіну + claim; SSR для share-превʼю/OG-теги), DB (міграція 0033_ticket_claim.sql)  ·  **Effort:** M  ·  **Залежності:** INIT-ROADMAP-SPEC — базовий spec-апдейт роадмапу (governing, §2), docs/spec.md — обовʼязкове фіксування нового стану 'unclaimed' ПЕРЕД кодом (rules §0/§2), email.rs / email-outbox seam — узагальнити на довільний recipient email (зараз order-scoped, 0026); АБО клієнтський share-URL шлях, telemetry.rs — розширення name-переліку (дешеве, наявний seam) для K-factor трекінгу gift-петлі, Google Sign-In робочий (TASK-AUTH-1) — для frictionless claim без акаунта; до його готовності claim через наявний auth-шлях, provider-neutral escrow seam — лишається недоторканим (гроші не рухаються); успадкувати frozen-перевірку з transfer_ticket, 0021_free_ticket_dedup / 0030_orders_ticket_unique — предикати оновити щоб unclaimed не спрацьовував хибно

**У scope:**
- Міграція 0033_ticket_claim.sql: tickets.status додає значення 'unclaimed' (як CHECK/enum-розширення), нові колонки claim_token TEXT UNIQUE NULL, claim_email CITEXT NULL, gifted_by UUID NULL REFERENCES users(id), gift_message TEXT NULL, claim_viewed_at TIMESTAMPTZ NULL, claimed_at TIMESTAMPTZ NULL, claim_expires_at TIMESTAMPTZ NULL
- Оновити transfer_ticket (tickets.rs:255): якщо email не резолвиться у users → НЕ падати, а перевести білет у 'unclaimed' з claim_token/claim_email/gifted_by/gift_message; існуючий отримувач — поведінка як раніше (owner_id одразу)
- Новий публічний (анонім-дозволений) endpoint GET /api/gift/:token — превʼю білета для незалогіненого: title, дата/місто, tier, ім'я дарувальника, gift_message, стан; ставить claim_viewed_at (одноразово) і логує gift_viewed
- Новий endpoint POST /api/gift/:token/claim (auth required, frictionless: клієнт спершу проходить Google Sign-In у тому ж екрані) — привласнює білет залогіненому користувачу: owner_id=me, status='active', очищає claim_token, ставить claimed_at; проходить age-gate; логує gift_claimed
- Endpoint POST /api/tickets/:id/gift-remind — дарувальник повторно надсилає лінк (rate-limited); логує gift_reminded
- Endpoint POST /api/tickets/:id/gift-revoke — дарувальник відкликає невитребуваний білет (тільки status='unclaimed'): білет повертається owner_id=дарувальник, status='active', очищення claim_*; логує gift_revoked
- Розширити my_tickets/TicketDto полями gift-статусу для відображення 'Надіслано/Переглянуто/Забрано' на боці дарувальника (для білетів status='unclaimed' що він подарував — окрема секція/DTO gifted_out)
- Email/share доставка лінка отримувачу через наявний email-seam (email.rs) — узагальнити outbox на довільний recipient email (не лише order-scoped) АБО повернути share-URL клієнту для месенджера; single-use short-TTL підписаний токен
- Anti-abuse: claim_token single-use, TTL (claim_expires_at, дефолт 14 днів), rate-limit на gift-remind і на створення gift на невідомий email (email/device cap)
- Оновлення docs/spec.md: новий стан білета + інваріанти unclaimed (сидіння НЕ звільняється поки не revoked/expired; escrow-звʼязок лишається за purchaser як у BACK-1)
- Тести: unit+integration (positive+negative+[SEC]) container-first, 100% coverage нового коду

**Поза scope:**
- Gift-at-checkout (checkout {gift:{recipient_email}}) — окрема задача TASK-CHECKOUT, тут лише post-purchase/free transfer шлях
- Guest checkout (оплата без акаунта) — окрема задача; тут gift працює для вже наявних (free або куплених) білетів
- Transfer по @username/контакту — окремо; тут email АБО share-лінк
- Wallet (.pkpass/Google Wallet), rotating QR — окремі SDD (QR-1)
- Реальний PSP/провайдерні зміни — гроші не рухаються цією задачею
- Автоматичне повернення сидіння по TTL-експірації через фоновий sweep — зарезервувати як окремий крок (тут expiry перевіряється лениво при claim/preview; фоновий reclaim — окремий тікет якщо потрібно)

**Модель даних:**
- Міграція api/migrations/0033_ticket_claim.sql — append-only, БЕЗ прямої мутації даних поза UPDATE self-heal
- tickets: розширити допустимі status значенням 'unclaimed' (поточні: active/used/returned/transferred; додати unclaimed). Якщо є CHECK constraint — ALTER; якщо TEXT без constraint — оновити код-валідацію
- tickets ADD COLUMN claim_token TEXT UNIQUE — формат 'GIFT-<uuid>'; NULL для звичайних білетів; single-use (очищається у NULL при claim/revoke)
- tickets ADD COLUMN claim_email CITEXT NULL — цільовий email подарунка (для повторної відправки/аудиту)
- tickets ADD COLUMN gifted_by UUID NULL REFERENCES users(id) ON DELETE SET NULL — хто подарував (для 'відкликати'/аудиту)
- tickets ADD COLUMN gift_message TEXT NULL — повідомлення дарувальника (валідація ≤500 char)
- tickets ADD COLUMN claim_viewed_at TIMESTAMPTZ NULL — коли отримувач вперше відкрив лінк ('Переглянуто')
- tickets ADD COLUMN claimed_at TIMESTAMPTZ NULL — коли забрав ('Забрано')
- tickets ADD COLUMN claim_expires_at TIMESTAMPTZ NULL — TTL лінка (дефолт now()+14d при створенні unclaimed)
- CREATE INDEX tickets_claim_token_idx ON tickets(claim_token) WHERE claim_token IS NOT NULL
- CREATE INDEX tickets_gifted_by_idx ON tickets(gifted_by) WHERE status='unclaimed'
- Інваріант: unclaimed-білет НЕ має owner_id отримувача (owner_id лишається дарувальника ДО claim, або вводиться sentinel — рішення ПРОСТЕ: owner_id=gifted_by поки unclaimed, доступ до e-ticket заблоковано доки status='unclaimed'); частковий unique index free-ticket (0021) не має спрацьовувати на unclaimed → додати status<>'unclaimed' до WHERE або лишити (бо price може бути 0) — ЯВНО оновити 0021-предикат новою міграцією щоб unclaimed не блокував
- telemetry_events: нові name-значення gift_sent, gift_viewed, gift_claimed, gift_reminded, gift_revoked, gift_signup (props: ticket_id, event_id, has_account:bool)

**API:**
- POST /api/tickets/:id/transfer {recipient_email?: string, recipient_handle?: string, message?: string} (auth) — ЗМІНЕНО: якщо email не знайдено у users → створити unclaimed з claim_token замість 404/Validation; повертає {ticket, claim_url?: string} (claim_url присутній лише для unclaimed). Existing-user шлях без змін (owner_id одразу, status active). Age-gate для existing user як раніше; для unclaimed — переноситься на момент claim
- GET /api/gift/:token (АНОНІМ дозволено, auth optional) — превʼю: {event:{title,starts_at,city,venue_address,poster_url,age_rating}, tier_name?, gifted_by_name, gift_message?, status:'unclaimed'|'claimed'|'expired'|'revoked', already_claimed:bool}. Side-effect: якщо перший перегляд — set claim_viewed_at=now(), log gift_viewed. 404 якщо токен невідомий; 410 якщо expired/revoked
- POST /api/gift/:token/claim (AUTH required) — привласнює: перевірка status='unclaimed' AND claim_expires_at>now(); age-gate meets_age(me, event.age_rating) → 409 з інлайн-підказкою якщо не проходить; UPDATE owner_id=me, status='active', claimed_at=now(), claim_token=NULL; log gift_claimed(has_account). 410 expired, 409 already_claimed/revoked, 404 unknown. Ідемпотентність: якщо вже claimed цим самим me → повернути 200 з білетом
- POST /api/tickets/:id/gift-remind (AUTH, owner=gifted_by) — тільки status='unclaimed'; повторно надсилає лінк на claim_email через email-seam; rate-limit (напр. ≤1/год, ≤5 всього); log gift_reminded. 409 якщо вже claimed
- POST /api/tickets/:id/gift-revoke (AUTH, owner=gifted_by) — тільки status='unclaimed'; повертає білет дарувальнику (status='active', очищення claim_*); log gift_revoked. 409 якщо вже claimed
- GET /api/me/tickets — ЗМІНЕНО TicketDto: додати gift-секцію/поля для власних unclaimed-подарунків: {gift_status: 'sent'|'viewed'|'claimed'|null, claim_email?, claim_url?, claim_viewed_at?, claimed_at?}. Забрані/активні білети без змін

**UI:**
- Web /gift/[token] (публічна, SSR, працює БЕЗ логіну): постер+назва+дата/місто, 'N подарував(ла) вам квиток', gift_message, кнопка 'Забрати квиток' → якщо не залогінений відкриває Google Sign-In inline і одразу викликає claim (frictionless). OG-теги для превʼю у месенджерах. Стани expired/revoked/already-claimed з відповідним копірайтом
- Web /me/tickets — секція/бейдж 'Подаровані' зі статусом Надіслано/Переглянуто/Забрано + кнопки 'Нагадати'/'Відкликати' для unclaimed
- Android E-Ticket / діалог 'Подарувати': поле email АБО share-лінк + повідомлення; після надсилання показ share-sheet із claim_url для месенджерів; білет переходить у 'Подаровано (очікує)' 
- Android MyTicketsScreen: секція 'Подаровані' зі статус-бейджем (Надіслано/Переглянуто/Забрано) + дії 'Нагадати'/'Відкликати поки не забрали'
- Android deep-link underground://gift/:token → превʼю-екран (працює для аноніма) → 'Забрати' → Google Sign-In → claim; після успіху → E-Ticket
- Копірайт станів: 'Надіслано — очікує', 'Переглянуто N дн тому', 'Забрано ✓', 'Термін дії лінка минув', 'Ви відкликали цей подарунок'

**Acceptance Criteria:**
- AC-1: transfer_ticket на email, ЩО Є в users → поведінка незмінна: owner_id негайно змінюється, status='active', 200. (регресія)
- AC-2: transfer_ticket на email, ЩО НЕ Є в users → білет переходить у status='unclaimed', генерується claim_token, claim_email збережено, claim_expires_at=now()+14d; відповідь містить claim_url; логується gift_sent. НЕ повертає помилку.
- AC-3: GET /api/gift/:token для валідного невитребуваного токена (анонім, без Authorization) → 200 з превʼю; при першому виклику ставиться claim_viewed_at і логується gift_viewed; повторний виклик не перезаписує claim_viewed_at.
- AC-4: POST /api/gift/:token/claim залогіненим користувачем → owner_id=me, status='active', claimed_at set, claim_token обнулено; логується gift_claimed; e-ticket доступний власнику.
- AC-5: Свіжозареєстрований користувач (signup під час claim) успішно забирає білет тим самим потоком; логується gift_signup + gift_claimed (has_account=false).
- AC-6: Дарувальник бачить у 'Мої квитки' статус подарунка, що коректно переходить Надіслано→Переглянуто→Забрано відповідно до claim_viewed_at/claimed_at.
- AC-7: gift-revoke на unclaimed → білет повертається дарувальнику (status='active'), claim_token недійсний; наступний GET /api/gift/:token → 410. gift-revoke на вже claimed → 409.
- AC-8: gift-remind повторно надсилає лінк на claim_email; понад rate-limit → 429; на claimed білет → 409.
- AC-9: Age-restricted подія: claim користувачем, що НЕ відповідає віку → 409 з інлайн-підказкою; owner/status не змінюються.
- AC-10: Термін лінка минув (claim_expires_at<now) → GET/claim повертають 410; сидіння не звільняється автоматично цією задачею (лениво, поза scope).
- AC-11: docs/spec.md оновлено новим станом 'unclaimed' та інваріантами ПЕРЕД мерджем коду; escrow-звʼязок покупця незмінний (гроші не рухаються).
- AC-12: unclaimed-білет НЕ зʼявляється у публічному e-ticket отримувача до claim і НЕ підпадає під free-ticket unique index (0021) хибно.

**Security-вектори (SEC):**
- SEC-1: claim_token — криптографічно випадковий (UUIDv4+ префікс), UNIQUE, single-use (обнуляється при claim/revoke), short-TTL (claim_expires_at). Не вгадуваний, не перевикористовуваний.
- SEC-2: GET /api/gift/:token НЕ розкриває PII отримувача чи дарувальника понад ім'я+повідомлення; не повертає qr_token/секрет сканування анонімному переглядачу (QR видається лише після claim власнику).
- SEC-3: claim захищений від race/подвійного привласнення — UPDATE ... WHERE status='unclaimed' AND claim_token=$1 RETURNING (атомарно, FOR UPDATE); паралельні claim → лише один виграє, інший 409.
- SEC-4: gift-remind/gift-revoke авторизовані лише для gifted_by (не будь-який user); чужий id → 403.
- SEC-5: Anti-abuse — rate-limit на створення gift на невідомий email та на gift-remind (email/device/IP cap) щоб лінк-фабрика не спамила; enumeration токенів throttled.
- SEC-6: Age-gate НЕ обходиться подарунком — перевірка meets_age виконується у момент claim (коли особа отримувача відома), а не пропускається як у прямому transfer до існуючого юзера.
- SEC-7: Escrow-інваріант: жоден gift/claim/revoke endpoint не викликає escrow.rs/провайдера і не змінює orders.escrow_status; білет під frozen/disputed order НЕ можна перевести в unclaimed (успадкувати наявну перевірку frozen з transfer_ticket).
- SEC-8: claim не дозволяє self-claim токена дарувальником для маніпуляції лічильниками; ідемпотентно, без побічного минта.
- SEC-9: Валідація вводу — gift_message ≤500 char, email нормалізований (lowercase/trim), відсутність HTML-інʼєкції у превʼю/email-шаблоні (escape).

**Test Matrix:**
- [POS] transfer до існуючого юзера → active, owner змінено (регресія)
- [POS] transfer до невідомого email → unclaimed + claim_token + gift_sent лог
- [POS] GET /api/gift/:token анонімом → превʼю + claim_viewed_at + gift_viewed
- [POS] claim залогіненим → active, claimed_at, e-ticket доступний, gift_claimed лог
- [POS] claim свіжим signup-юзером → gift_signup+gift_claimed(has_account=false)
- [POS] дарувальник бачить перехід Надіслано→Переглянуто→Забрано
- [POS] gift-remind у межах rate-limit → email надіслано, gift_reminded
- [POS] gift-revoke на unclaimed → повернуто дарувальнику, токен недійсний
- [NEG] GET/claim невідомого токена → 404
- [NEG] GET/claim простроченого токена → 410
- [NEG] claim вже забраного білета іншим → 409
- [NEG] revoke/remind на claimed → 409
- [NEG] transfer білета собі → 400 (незмінна валідація)
- [NEG] claim на age-restricted без відповідного віку → 409 з підказкою
- [NEG] gift_message >500 char → 400
- [SEC] claim іншим паралельним запитом (race) → лише один успіх (409 іншому)
- [SEC] remind/revoke не-власником (не gifted_by) → 403
- [SEC] анонім GET /api/gift/:token не отримує qr_token
- [SEC] rate-limit перевищено на gift-remind → 429
- [SEC] transfer в unclaimed білета під frozen-order → 409 (escrow-інваріант)
- [SEC] token enumeration/brute — throttling спрацьовує
- [SEC] unclaimed не порушує free-ticket unique index (0021)

---

## SDD-018 — Frictionless gift/claim-петля на єдиному signed-token seam (claimable gift-link + публічна OG-landing + inline auth + вихідний share-CTA)

**Мета:** Реалізувати наскрізну віральну петлю "перейшов за лінком без акаунта → побачив цінність → забрав подарунок → зареєструвався тут же → сам поділився" на ОДНОМУ signed-token seam для free та paid квитків, Android+web. Ціль — підняти K-factor (link_created→opened→value_viewed→claimed→registered→re-shared) ≥ 0.5 і частку активацій з gift-links. Рішення тримати простим: розширення наявних tickets/telemetry/auth seam-ів, без нового платіжного коду (provider-agnostic: комісія з подарованих/guest/free не береться взагалі).

**Поверхні:** API (Rust/Axum), Android (Kotlin/Compose), Web (public landing /g,/t), PM/MKT (tracking plan, OG-копірайт)  ·  **Effort:** L  ·  **Залежності:** INIT-TICKET-UNCLAIMED (стан 'unclaimed' у tickets — постачається міграцією 0034 у цьому ж епіку, якщо ще не існує)., INIT-DEEPLINKS (Android AppLinks/deeplink інфраструктура для /g,/t,underground://gift)., INIT-CHANNELS (share-sheet/канали для вихідного share-CTA; мінімально — системний share)., INIT-ANTIABUSE (rate-limit/device-cap seam — SEC-4 є умовою готовності, не окремою задачею)., Наявні seam-и: tickets.rs (mint/fetch/decrement, transfer BACK-1 policy), telemetry.rs (log), auth.rs (issue_token/Google), email_outbox (0026), payments NoopProvider (комісія=0 на gift/free)., Оновлення docs/spec.md ПЕРЕД кодом (rules §0/§2): стан unclaimed, gift-token lifecycle, no-commission-on-gift/guest/free.

**У scope:**
- Claimable gift-link first-class: POST /api/tickets/:id/gift-link мінтить одноразовий signed-token, переводить ЧИННИЙ квиток у status='unclaimed' і зберігає donor_id; ескроу дарувальника (для paid) лишається held/clearing на donor до фактичного claim або expiry.
- Єдиний token seam: HMAC-SHA256 signed opaque token {ticket_id, kind, nonce, exp}, single-use, short-TTL (default 7 днів, конфіг), збережений у новій таблиці gift_tokens; той самий формат обслуговує і подарунок (kind=gift), і публічний перегляд квитка (kind=view для /t).
- Публічний GET /api/gift/:token (no-auth) — повертає OG-багатий payload про подію+tier+donor+статус (де/коли/tier/ціна=подарунок/ім'я дарувальника), QR НЕ повертається до claim.
- Публічні web-сторінки /g/[token] (gift landing, OG/Twitter meta) і /t/[token] (guest ticket view) — рендерять цінність ДО логіну.
- Inline auth на тій самій сторінці: POST /api/gift/:token/claim з тілом {auth: google_id_token} АБО passwordless email-код (POST /api/auth/email/start {email}+POST /api/auth/email/verify {email,code} → app-JWT), без редіректу геть зі сторінки.
- Атомарний claim: перший авторизований claim переводить ticket unclaimed→active, owner_id=claimer, споживає token (used_at), для paid — донор-ескроу лишається на донорі (даруємо ENTRY, право disputes у донора, як у наявному transfer BACK-1).
- Обов'язковий вихідний share/gift-CTA на екрані успіху (Android success + web /g claimed state): 'Візьми друга — подаруй другий' → генерує НОВИЙ gift-link (якщо є ще квитки) або share-лінк події.
- Android: deeplink underground://gift/:token + https AppLinks на /g/ і /t/; success-екран із share-CTA; кнопка 'Подарувати' у Мої квитки викликає gift-link.
- Anti-abuse обгортка: single-use short-TTL token + rate-limit на створення/claim + device/email-cap; revoke gift-link (DELETE) власником поки unclaimed.
- Telemetry-події всієї петлі через наявний telemetry seam.
- Оновлення docs/spec.md (стан 'unclaimed', gift-token lifecycle, no-commission-on-gift) ПЕРЕД кодом.

**Поза scope:**
- Реальний PSP/live-платіж, ledger, зміна escrow.rs формул комісії (seam лишається NoopProvider; комісія з gift/guest/free = 0 за визначенням).
- Paid GUEST checkout без акаунта (купівля за гроші анонімом) — окрема ініціатива; тут paid-gift дарує вже КУПЛЕНИЙ донором квиток.
- Wallet (Apple/Google), print-stylesheet, ICS — поза цим епіком.
- Повноцінна рекомендаційна стрічка/Афіша-ранжування, DM-share композер, followers/following екрани.
- Staff/door-scan делегування (окремий P0-епік INIT-CHANNELS/staff).
- Rotating/HOTP QR — тут QR лишається наявний статичний qr_token, просто прихований до claim.
- SMS/телефон-верифікація — passwordless тільки email-код + Google one-tap.

**Модель даних:**
- Міграція 0033_gift_tokens.sql: CREATE TABLE gift_tokens(id UUID PK DEFAULT gen_random_uuid(), ticket_id UUID NOT NULL REFERENCES tickets(id) ON DELETE CASCADE, kind TEXT NOT NULL CHECK(kind IN ('gift','view')), token_hash TEXT NOT NULL UNIQUE, donor_id UUID REFERENCES users(id) ON DELETE SET NULL, created_by UUID NOT NULL REFERENCES users(id), expires_at TIMESTAMPTZ NOT NULL, used_at TIMESTAMPTZ NULL, claimed_by UUID NULL REFERENCES users(id), revoked_at TIMESTAMPTZ NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now()). Зберігаємо ТІЛЬКИ hash токена (не raw), як password_resets.
- INDEX gift_tokens_ticket_idx ON gift_tokens(ticket_id); partial UNIQUE gift_tokens_active_gift ON gift_tokens(ticket_id) WHERE kind='gift' AND used_at IS NULL AND revoked_at IS NULL — максимум один активний gift-link на квиток одночасно.
- Міграція 0034_ticket_unclaimed.sql: розширити дозволені tickets.status значенням 'unclaimed'; додати колонку donor_id UUID NULL REFERENCES users(id) (для paid: хто утримує ескроу/право disputes після подарунка). Оновити партіал-unique free-ticket index (0021), щоб 'unclaimed' не рахувався за 'active' free-квиток одержувача (щоб claim не конфліктнув з наявним free-квитком до моменту переходу в active — перевірка робиться в handler'і).
- Міграція 0035_email_codes.sql: CREATE TABLE email_login_codes(email TEXT NOT NULL, code_hash TEXT NOT NULL, expires_at TIMESTAMPTZ NOT NULL, attempts INT NOT NULL DEFAULT 0, created_at TIMESTAMPTZ NOT NULL DEFAULT now(), PRIMARY KEY(email)); TTL 10 хв, max 5 attempts. Reuse наявного email_outbox для доставки коду.
- Реюз telemetry_events (0..) — нових таблиць під tracking НЕ створюємо.

**API:**
- POST /api/tickets/:id/gift-link (auth) → {url, token, expires_at, kind:'gift'}. Прекондиції: caller=owner, ticket.status='active', НЕ frozen (спір), не past-event. Side-effects: status active→unclaimed, donor_id=owner (paid), мінт gift_token (revoke попереднього активного gift-link на цей квиток). 409 якщо frozen/used/past; 404 чужий/невідомий.
- GET /api/gift/:token (NO auth) → {ticket_status, event:{id,title,starts_at,ends_at,city,venue_address,poster_url,timezone}, tier:{name,price,currency}, is_gift:true, donor_name, claimable:bool, expired:bool, already_claimed:bool}. QR/qr_token НЕ віддається. 404 на невалідний/невідомий token (без розкриття причини).
- POST /api/gift/:token/claim (NO Bearer обов'язково) body: {auth:{google_id_token}} | {} якщо вже є Bearer. Потік: (a) якщо Bearer присутній → claim під ним; (b) якщо google_id_token → verify→upsert user→issue JWT→claim; повертає {token(JWT), user, ticket}. Атомарно: SELECT gift_token FOR UPDATE; перевірити not used/revoked/expired; ticket.status='unclaimed'; set ticket.status='active', owner_id=claimer; gift_token.used_at=now(), claimed_by=claimer; заборонити claim власного подарунка (claimer==donor → 409). Telemetry gift_claimed.
- POST /api/auth/email/start {email} (NO auth) → 204; мінт 6-значного коду, rate-limited, лист через email_outbox. НЕ розкриває існування акаунта.
- POST /api/auth/email/verify {email, code} (NO auth) → {token(JWT), user}; upsert user (passwordless, password_hash NULL) при першому вході; max 5 attempts, спалення коду. Used both для claim та звичайного логіну.
- DELETE /api/tickets/:id/gift-link (auth, owner) → 204; revoke активний gift-link, ticket.status unclaimed→active назад до власника. 409 якщо вже claimed.
- POST /api/tickets/:id/share-link (auth) → {url} для kind='view' /t/[token] (публічний перегляд без claim) — легкий шер квитка/події.
- Реюз наявного POST /api/telemetry для клієнтських подій воронки.

**UI:**
- Web /g/[token] (gift landing): SSR з OG/Twitter meta (og:title=подія, og:image=poster, og:description='<Donor> дарує тобі квиток'); герой-картка де/коли/tier/місто, розмитий QR-плейсхолдер, банер 'Подарунок від <ім'я>'; єдина primary-кнопка 'Забрати'; під нею inline auth: Google one-tap + 'або email-код'; 'інші способи' (email/@username) — згорнуто вторинним. Стани: claimable / expired / already_claimed / revoked.
- Web claimed success (той самий /g): 'Готово — квиток твій', кнопка 'Дивитись квиток' (/t або /me/tickets), і ОБОВ'ЯЗКОВИЙ share-CTA 'Візьми друга — подаруй другий' (генерує новий gift-link якщо є квитки, інакше share події).
- Web /t/[token] (guest ticket view, kind='view'): деталі + QR лише якщо перегляд авторизований власником; для гостя — цінність без QR + CTA залогінитись.
- Android Мої квитки: пункт 'Подарувати' на active-квитку → gift-link → системний share-sheet (месенджери/соцмережі). Стан 'unclaimed' показує бейдж 'очікує на отримувача' + 'Скасувати подарунок'.
- Android claim через deeplink underground://gift/:token та https AppLinks (/g,/t): екран landing (цінність+Google one-tap/email-код), success-екран з тим самим share-CTA 'Візьми друга — подаруй другий'.
- Android AuthScreen: додати passwordless email-код flow (start→verify) поряд з Google one-tap; email/password лишити за 'інші способи' (узгоджено з P0 Google-only напрямком).

**Acceptance Criteria:**
- AC-1: Власник active-квитка викликає POST /api/tickets/:id/gift-link і отримує https-url з opaque signed token; квиток стає status='unclaimed', donor_id=owner (paid); GET того самого квитка більше не показує його як 'віддавабельний' повторно (partial-unique блокує другий активний gift-link).
- AC-2: GET /api/gift/:token без будь-якого Authorization повертає деталі події+tier+donor_name і НЕ містить qr_token у відповіді; поле claimable=true поки не used/expired/revoked.
- AC-3: Анонім відкриває /g/[token], бачить OG-preview у месенджері (og:image=poster) і повну картку цінності ДО логіну; auth-wall з'являється лише при натисканні 'Забрати'.
- AC-4: Перший авторизований claim (Google one-tap АБО email-код, на тій самій сторінці, без редіректу геть) переводить ticket unclaimed→active, owner_id=claimer, повертає app-JWT+user+ticket; token стає used (single-use).
- AC-5: Другий claim того самого token → 409/404 (already_claimed); expired token → 410/404 expired; revoked → 404. 'Хто перший забрав' виграє — гонка двох claim-ів дає рівно один active-квиток (FOR UPDATE).
- AC-6: Для paid-подарунка комісія НЕ нараховується на подарований шлях; ескроу лишається прив'язаним до donor_id (право disputes у донора), одержувач отримує ENTRY-квиток без нового order/провайдера.
- AC-7: Free-квиток дарується так само (unclaimed→active у одержувача) без виклику жодного провайдера; повторний free-квиток на ту саму подію в одержувача не дублюється (перевірка в claim handler → 409 з чистим повідомленням, token НЕ спалюється).
- AC-8: Екран успіху (Android + web) ЗАВЖДИ показує вихідний share/gift-CTA; клік генерує новий gift-link (якщо є ще квитки) або share-лінк події — і це фіксується telemetry-подією re_share.
- AC-9: Власник може DELETE gift-link поки unclaimed → квиток повертається в active до нього; після claimed DELETE → 409.
- AC-10: Passwordless: email/start мінтить код (10 хв, доставка через email_outbox), verify видає JWT і створює акаунт при першому вході; >5 невдалих спроб блокує код.
- AC-11: Уся воронка інструментована: link_created, link_opened, value_viewed, claim_started, claim_succeeded, registered (новий user), re_shared — кожна подія у telemetry_events з ticket_id/token-nonce (не raw token).

**Security-вектори (SEC):**
- SEC-1: Token — opaque, HMAC-SHA256 підпис на server-secret; у БД лише token_hash (як password_resets), raw ніде не логується/не в telemetry; неможливо підробити/вгадати (128-bit nonce).
- SEC-2: Single-use — used_at + FOR UPDATE гарантують рівно один claim; конкурентні claim/повтор webhook = безпечний no-op, без подвійного квитка чи подвійного списання місць.
- SEC-3: Short-TTL — expires_at (default 7 днів), протерміновані токени не claimable; фонове прибирання/лінива перевірка.
- SEC-4: Rate-limit + cap — на gift-link creation (per user), на claim/email-start (per IP+email+device), email-код max 5 attempts; захист від масового мінту/брутфорсу коду/enumeration.
- SEC-5: GET /api/gift/:token не витікає qr_token і не розрізняє 'не існує' vs 'протермінований' надлишковими деталями (мінімізація enumeration); donor_name — тільки display name, не email.
- SEC-6: Age-gate зберігається на claim (як у наявному transfer) — не можна забрати квиток на 18+ подію, не відповідаючи рейтингу; frozen(спір)-квиток не можна подарувати.
- SEC-7: Не можна claim власний подарунок (claimer==donor→409); не можна gift чужий/used/past квиток (owner+status guard, як transfer).
- SEC-8: Passwordless код — hash у БД, TTL, attempt-cap, не розкриває існування акаунта на /start; JWT видається лише після verify; узгоджено з tokens_valid_after watermark.
- SEC-9: AppLinks/deeplink — token валідується сервером, клієнт нічого не довіряє з URL; https-домен verified (assetlinks) щоб уникнути перехоплення gift-лінку іншим застосунком.

**Test Matrix:**
- [POS] gift-link на free active-квиток → 200, status unclaimed, token валідний.
- [POS] gift-link на paid active-квиток → status unclaimed, donor_id=owner, ескроу лишається на донорі.
- [POS] GET /api/gift/:token анонімно → деталі без qr_token, claimable=true.
- [POS] claim через Google one-tap новим користувачем → створює user, JWT, ticket active у claimer, token used.
- [POS] claim через email-код (start→verify) новим користувачем → як вище.
- [POS] claim авторизованим наявним користувачем (Bearer) → active у claimer.
- [POS] DELETE gift-link поки unclaimed → quiток назад active у власника.
- [POS] success-CTA генерує новий gift-link коли є ще квитки → re_shared telemetry.
- [NEG] gift-link на used/returned/past квиток → 409.
- [NEG] gift-link на чужий квиток → 404.
- [NEG] gift-link на квиток, що вже має активний gift-link → 409 (partial-unique).
- [NEG] claim протермінованого token → 410/404 expired.
- [NEG] claim вже claimed token → 409/404.
- [NEG] claim власного подарунка (claimer==donor) → 409.
- [NEG] claim free-квитка користувачем, що вже має free-квиток на цю подію → 409, token НЕ спалено.
- [NEG] claim на age-restricted подію користувачем, що не відповідає рейтингу → 409.
- [NEG] gift-link на frozen(спір) квиток → 409.
- [NEG] email-verify з невірним кодом → 401; >5 спроб → код заблоковано.
- [SEC] підроблений/зіпсований підпис token → 404, без сигналу про причину.
- [SEC] гонка: два одночасних claim одного token → рівно один active-квиток, другий 409.
- [SEC] enumeration: невідомий vs протермінований token не розрізняються за timing/детальністю.
- [SEC] rate-limit: N gift-link/email-start за хвилину понад ліміт → 429.
- [SEC] raw token/код не потрапляє в telemetry_events/логи (перевірка props не містить token).
- [SEC] GET /api/gift/:token ніколи не повертає qr_token у жодному стані.
- [SEC] passwordless /start не розкриває існування акаунта (однакова відповідь 204).

---

## SDD-INIT-DEEPLINKS — Верифіковані universal-links + OG/Twitter-прев'ю + guest gift-claim для всіх share-об'єктів

**Мета:** Замкнути віральну петлю для аудиторії БЕЗ акаунта: будь-яке share-посилання (подія, gift-квиток, RSVP-інвайт) — це верифікований universal https-лінк, що (а) відкриває Android-апку якщо встановлена, інакше web-landing з чітким CTA; (б) у месенджері/соцмережі показує брендоване OG/Twitter-прев'ю (1200×630); (в) дозволяє анонімному отримувачу забрати/побачити подарунок ДО логіну і зареєструватися тут же в пару кліків. Provider-agnostic: жоден грошовий seam не чіпається (gift передає лише ENTRY, як нинішній transfer_ticket). Мета — підняти K-factor share→claim→signup→re-share ≥ 0.5 і конверсію visit→signup.

**Поверхні:** Android, Web, API  ·  **Effort:** M  ·  **Залежності:** Публічний web-client host (public роути в admin/ Next app або окремий web/ — наразі існує ТІЛЬКИ admin/; потрібен публічний неавторизований layout — перетин з TASK-016 public-web-client), Наявний signed-config: env-каркас (config.rs from_env) — додати PUBLIC_WEB_ORIGIN, DEEPLINK_HOST, SHARE_TOKEN_SECRET, SHARE_LINK_RATE_LIMIT (потребує апруву на нові env, §10), transfer_ticket money-policy (api/src/tickets.rs) — переиспользовується як seam для gift ENTRY-передачі; escrow.rs НЕ чіпається, telemetry::log seam (api/src/telemetry.rs) — розширення новими name; без нового екрана, meets_age (api/src/users.rs) для age-gate у claim, Android release keystore SHA-256 fingerprint для assetlinks.json (потрібен фактичний signing-cert; dev+prod fingerprints), Google-only auth flow (SDD-002) має бути робочим для inline signup у guest-claim (нині Google — заглушка; блокує frictionless signup)

**У scope:**
- Android: autoVerify intent-filter (https, host з env) + BROWSABLE + окремий underground:// scheme-фолбек у AndroidManifest; DeepLinkRouter, що резолвить /events/:id, /g/:token, /i/:token у відповідні екрани; graceful-фолбек на web якщо шлях невідомий апці
- Web (public client, admin/ Next.js app-router): server-rendered роути /events/[id], /g/[token], /i/[token] з generateMetadata (OG+Twitter+canonical); web-landing з CTA 'Відкрити в застосунку / Продовжити у браузері'
- Динамічний opengraph-image route (Next ImageResponse, 1200×630) для тих самих трьох шляхів: брендований cover з постером-фолбеком, дата·місто, аватар-стек 'N друзів іде', для gift '‹Ім'я› дарує тобі квиток'
- Статичні well-known файли: /.well-known/assetlinks.json (Android App Links) + /.well-known/apple-app-site-association (заготовка під iOS, served з правильним content-type без .json-розширення)
- API: єдиний signed-token seam для share-об'єктів (gift-ticket token, event-invite token) — HMAC-signed, single-use для gift, short-TTL, з rate-limit та email/device-cap
- API: POST /api/tickets/:id/gift-link → створює guest-gift signed token (без вимоги, щоб отримувач мав акаунт); GET /api/g/:token → публічні дані подарунка для landing/OG (без auth); POST /api/g/:token/claim → guest або залогінений забирає квиток
- API: GET /api/events/:id вже є — розширити публічною OG-моделлю (cover, дата, місто, friends-going avatars) без auth
- Телеметрія на існуючому seam (telemetry::log): share_link_created, share_link_visited, gift_link_claimed, guest_signup_from_link, re_share — для KPI share-CTR і visit→signup
- Конфіг: PUBLIC_WEB_ORIGIN / DEEPLINK_HOST env (без хардкоду хоста/порту, §10)

**Поза scope:**
- iOS-апка (лише заготовка AASA-файлу, жодного Swift-коду)
- Wallet-паси (Apple/Google Wallet) — окрема задача
- Повноцінний public web client як продукт (лише 3 share-роути + landing; решта SEO-каталогу — TASK-016)
- Paid guest-checkout / gift-at-checkout грошовим провайдером (окремі P1-задачі; тут gift лише для вже випущеного free/paid квитка, як transfer_ticket)
- Ротація/HOTP QR-токена квитка (окремо)
- Email-доставка e-ticket (magic-link claim повертає в web/app; лист — окремо)
- Repost/DM внутрішній share (тут — системний OS-share лінка)

**Модель даних:**
- Нова таблиця share_tokens(id UUID PK, kind TEXT CHECK IN ('gift','event_invite'), token TEXT UNIQUE NOT NULL, ticket_id UUID NULL REFERENCES tickets(id), event_id UUID NULL REFERENCES events(id), created_by UUID REFERENCES users(id), recipient_email TEXT NULL, message TEXT NULL, expires_at TIMESTAMPTZ NOT NULL, claimed_at TIMESTAMPTZ NULL, claimed_by UUID NULL REFERENCES users(id), created_at TIMESTAMPTZ DEFAULT now()) — міграція 00NN_share_tokens.sql
- Індекси: UNIQUE(token); частковий idx ON (created_by, created_at) для rate-limit; частковий idx ON (recipient_email, created_at) для email-cap
- token — URL-safe base64 з HMAC (secret із env SHARE_TOKEN_SECRET); формат UG-<random>.<sig>; single-use для kind='gift' (claimed_at NOT NULL → 409); event_invite може бути multi-use до expires_at
- telemetry_events (існує) — нових колонок не треба; лише нові name-значення + props {token_kind, event_id, has_account}
- Незворотні контракти (принцип): суми в minor-units, event_timezone для дат в OG — використовувати наявні поля events, нових грошових полів не вводити

**API:**
- POST /api/tickets/:id/gift-link (auth, owner) → {url, token, expires_at}; створює share_tokens(kind='gift'); квиток лишається active у власника до claim; body optional {recipient_email?, message?}; 409 якщо квиток не active/під спором (frozen escrow) — reuse логіки transfer_ticket
- GET /api/g/:token (NO auth) → {kind, event:{id,title,cover_url,starts_at,city,timezone}, gifter:{name,avatar_url}, message, friends_going:[{avatar_url}], claimable:bool, expired:bool} — живить landing+OG; 404 невідомий, 410 expired
- POST /api/g/:token/claim → залогінений: mint/transfer ENTRY отримувачу (reuse transfer_ticket money-policy: лише entry, escrow лишається за покупцем), 200 {ticket_id}; анонім без auth: 401 з {signup_required:true, token} → клієнт веде на Google Sign-In і повторює claim (token у стані); single-use → 409 already_claimed; вік-гейт як у transfer_ticket (meets_age)
- POST /api/events/:id/invite-link (auth) → {url, token, expires_at}; share_tokens(kind='event_invite', multi-use)
- GET /api/i/:token (NO auth) → публічна OG-модель події (як GET /api/events/:id public subset) для landing/OG
- GET /api/events/:id — розширити відповідь публічним OG-підмножиною полів (cover_url, city, timezone, friends_going avatars) без зміни наявного контракту (адитивно, не breaking §10)
- POST /api/telemetry (існує) — нові events: share_link_created|share_link_visited|gift_link_claimed|guest_signup_from_link|re_share
- Web-роути (Next, не /api): GET /.well-known/assetlinks.json (application/json), GET /.well-known/apple-app-site-association (application/json, БЕЗ розширення), GET /events/[id]/opengraph-image, /g/[token]/opengraph-image, /i/[token]/opengraph-image (image/png 1200×630)

**UI:**
- Web /events/[id] (SSR): OG/Twitter/canonical метадані + JSON-LD Event; hero-cover, дата·місто, 'N друзів іде' аватар-стек; CTA 'Відкрити в застосунку' (smart-banner) + 'Отримати квиток'
- Web /g/[token] (SSR guest-landing): '‹Ім'я› дарує тобі квиток на ‹Подія›', cover, дата·місто, message; primary CTA 'Забрати подарунок' → Google Sign-In inline → claim; стан expired/already_claimed
- Web /i/[token] (SSR invite-landing): подія + 'Тебе запрошує ‹Ім'я›', CTA 'Дивитись подію / Встановити застосунок'
- opengraph-image (3 варіанти): брендований фон (accent #2F6BFF), постер-фолбек якщо cover відсутній, wordmark UNDERGROUND, дата·місто, аватар-стек, gift-варіант з іменем дарувальника
- Android: системний share-sheet генерує universal https-URL (ніколи голий underground://); DeepLinkRouter відкриває EventDetails/GiftClaim/EventInvite; GiftClaimScreen для залогіненого — 'Забрати', для гостя апка не показується (лінк веде у web-landing)
- Android AndroidManifest: intent-filter android:autoVerify=true з BROWSABLE+DEFAULT на https host; окремий underground:// scheme-filter як внутрішній фолбек

**Acceptance Criteria:**
- AC-1: Клік по https share-лінку події на пристрої зі встановленою (verified) апкою відкриває EventDetails без chooser-діалогу; без апки — відкриває web /events/[id]
- AC-2: /.well-known/assetlinks.json віддається з application/json і містить package_name та sha256_cert_fingerprints; adb 'pm verify-app-links' статус = verified для host
- AC-3: Розшарений у Telegram/Viber/Messenger/X лінк усіх трьох типів показує прев'ю з правильним title, description і 1200×630 зображенням (og:image + twitter:card=summary_large_image)
- AC-4: opengraph-image для gift містить рядок '‹Ім'я дарувальника› дарує тобі квиток' і назву події; для події без cover — брендований постер-фолбек, без 500
- AC-5: Анонім відкриває /g/:token, бачить подарунок і подію ДО будь-якого логіну (auth-wall лише на 'Забрати')
- AC-6: Анонім тисне 'Забрати' → Google Sign-In → claim успішний → квиток у 'Мої квитки'; телеметрія фіксує guest_signup_from_link + gift_link_claimed із зв'язком до token
- AC-7: gift-token single-use: повторний claim → 409 already_claimed; після expires_at → 410; UI landing показує людський стан
- AC-8: gift-link для квитка під спором (escrow frozen) → 409, як у transfer_ticket; вік-обмеження події застосовується до отримувача (claim від невідповідного віку → 409)
- AC-9: Жоден новий endpoint (gift-link/claim/invite) не викликає escrow.rs/payments; gift передає лише ENTRY, order/escrow лишаються за початковим покупцем (money-policy як transfer_ticket)
- AC-10: apple-app-site-association віддається за /.well-known/ з content-type application/json і БЕЗ розширення .json
- AC-11: Хост/origin беруться з env (PUBLIC_WEB_ORIGIN/DEEPLINK_HOST); жодного хардкоду домену/порту в коді (§10)
- AC-12: KPI-події (share_link_created/visited, gift_link_claimed, guest_signup_from_link, re_share) пишуться через telemetry::log і не ламають основну дію при збої запису

**Security-вектори (SEC):**
- SEC-1: Token forgery — токен HMAC-підписаний (SHARE_TOKEN_SECRET), сервер верифікує підпис ПЕРЕД будь-яким читанням БД; невалідний підпис → 404 (не 401, щоб не розкривати існування)
- SEC-2: Guessing/enumeration — токен ≥128 біт ентропії, невгадуваний; GET /api/g|/i повертає однакову 404 для неіснуючого і невалідного
- SEC-3: Replay/double-claim — claim у транзакції з FOR UPDATE на share_tokens; claimed_at set атомарно; повторний → 409 (single-use для gift)
- SEC-4: Rate-limit створення лінків — cap на created_by за вікно (env SHARE_LINK_RATE_LIMIT) і email-cap на recipient_email (проти масового спаму інвайтами)
- SEC-5: Abuse guest-claim — device/email-cap на claim з одного джерела; захист від фарму акаунтів через масові gift-клейми
- SEC-6: OG-image SSRF/injection — cover_url лише з власного uploads-домену (allowlist), імена/повідомлення екрануються в ImageResponse (без HTML-інжекту в SVG-текст)
- SEC-7: PII exposure — GET /api/g/:token віддає лише ім'я+аватар дарувальника і публічні поля події, НЕ email отримувача, НЕ qr_token, НЕ order/escrow дані
- SEC-8: Open redirect — web-landing CTA веде лише на власний scheme/known internal path; underground:// deeplink валідує шлях в allowlist перед навігацією
- SEC-9: assetlinks/AASA не витікають зайвого — лише package_name+fingerprint (assetlinks) і appID+paths (AASA), жодних секретів
- SEC-10: Age-gate bypass — claim застосовує meets_age до отримувача так само, як transfer_ticket, щоб gift не обходив віковий гейт checkout/reserve

**Test Matrix:**
- [POS] assetlinks.json та AASA віддаються з правильним content-type і схемою
- [POS] gift-link створення власником active-квитка → 200 з url+token, рядок у share_tokens
- [POS] GET /api/g/:token без auth → публічна модель подарунка; claim залогіненим → квиток у нового власника; телеметрія записана
- [POS] guest-flow: 401 signup_required → після Google-signin повторний claim → 200
- [POS] OG-image рендериться 200/png 1200×630 для події з cover і без cover (постер-фолбек)
- [POS] verified App Link відкриває апку; невідомий шлях → web-фолбек
- [NEG] невідомий/невалідний token → 404 (однаково); expired → 410; already-claimed gift → 409
- [NEG] gift-link для квитка не-власником / не-active / під спором (frozen) → 403/409
- [NEG] claim отримувачем, що не відповідає age_rating → 409
- [NEG] OG-image для неіснуючого event/token → безпечний фолбек-плейсхолдер, не 500
- [SEC] підроблений HMAC-підпис token → 404; enumeration неіснуючого → та сама 404
- [SEC] double-claim гонка (2 паралельні claim) → рівно один 200, інший 409 (FOR UPDATE)
- [SEC] перевищення rate-limit створення лінків / email-cap → 429
- [SEC] перевищення device/email-cap на guest-claim → 429
- [SEC] GET /api/g/:token не містить email отримувача, qr_token, order/escrow полів
- [SEC] cover_url поза uploads-allowlist у OG-роуті ігнорується/фолбек (анти-SSRF)
- [SEC] спецсимволи в gifter name/message не ламають і не інжектять SVG/HTML в OG-image
- [SEC] жоден gift/invite-endpoint не змінює escrow_status/orders (перевірка знімком БД до/після)

---

## SDD-CHANNELS-1 — Робочі lifecycle-канали: FCM push (HTTP v1) + transactional email за єдиним seam

**Мета:** Замкнути реальну доставку двох lifecycle-каналів, які сьогодні є TODO-заглушками (main.rs push_worker + email_worker логують intent і одразу ставлять watermark). Зробити канали справжнім P0 hard-prerequisite: (1) FCM HTTP v1 send з OAuth2 access-token, мінтованим із service-account key; (2) transactional email за єдиним провайдер-агностичним seam ChannelProvider (дзеркало payment-seam: trait + Noop/Log/SendGrid impl + registry за env), обов'язковий для квитка/receipt/refund/guest magic-code; (3) health-check доставки в admin. Без цього gift-delivery, receipt, guest magic-code, event-reminder, «друзі йдуть», follow-new-event — no-op, а web-guest/SEO-трафік взагалі без каналу доставки квитка. Ціль north-star: розблокувати guest→registered activation і retention D7/D30, не додаючи зайвих екранів (tracking — розширення telemetry-seam).

**Поверхні:** API  ·  **Effort:** M  ·  **Залежності:** Наявна outbox-інфраструктура: push.rs (pending/mark_pushed, FcmConfig), email.rs (pending/mark_sent/build_confirmation, EmailConfig), device_tokens (0016), orders.email_sent (0026), notifications.pushed_at (0016) — розширюються, не переписуються, Крейт reqwest (HTTP-клієнт) + rustls — ДОДАТИ в api/Cargo.toml (зараз відсутній; є лише jsonwebtoken=9, який покриває RS256 JWT-assertion для OAuth2), jsonwebtoken=9 (наявний) для мінту SA JWT-assertion, Payment-provider-seam як референс-патерн (docs/analysis/2026-06-29-product-changes-v2.md §2): trait+Noop/Test+registry — дзеркалимо для EmailChannel, Guest/gift signed-token seam (INIT gift/guest-loop) — guest_magic_code email споживає його; якщо seam ще не існує, magic-code-шаблон приймає токен ззовні (не блокує транспорт), але claim-flow — залежна задача, docs/spec.md оновлення (rules §2) + TASK-###/SDD-### lifecycle (rules §3/§12) ПЕРЕД кодом, НЕ залежить від реального PSP (Фаза 4) — канали доставляють і на Noop/Test-платежах

**У scope:**
- FCM HTTP v1: POST https://fcm.googleapis.com/v1/projects/{project_id}/messages:send з Bearer OAuth2 access-token (scope https://www.googleapis.com/auth/firebase.messaging), мінтованим локально з SA-key (RS256 JWT assertion → token endpoint), з кешуванням токена до exp-60s
- Заміна log-only циклу в push_worker (main.rs:139) на реальний виклик; per-token результат, а не сліпий batch-mark
- Єдиний seam email: trait EmailChannel { send(EmailMsg) -> Result<Delivery> } + impl NoopEmail (dev/Noop), LogEmail (CI-детермінований), SendGridEmail (POST https://api.sendgrid.com/v3/mail/send, Bearer, from cfg.from); registry резолвить за env (EMAIL_PROVIDER=noop|log|sendgrid), дзеркалить payment-provider-registry
- Розширення email-outbox з одного kind (order confirmation) на transactional-типи: ticket (mint), receipt, refund, guest_magic_code — усі за одним seam
- Заміна log-only циклу в email_worker (main.rs:172) на реальний виклик через seam; per-message результат
- Retry/backoff та dead-letter для обох outbox: NULL watermark лишається при hard-fail; додати attempts, next_attempt_at, last_error у notifications та orders(email) / окрему email_outbox якщо потрібно розширити типи; expo-backoff з cap; drop stale FCM-токена при UNREGISTERED/NOT_FOUND (видалити рядок device_tokens)
- Admin health-check доставки: GET /api/admin/channels/health → делівері-статистика за 24г (push/email: sent, failed, pending, oldest_pending_age_s, provider, configured:bool) + last_error; POST /api/admin/channels/test {channel, target} для одноразового test-send
- Env-каркас: додати FCM_PROJECT_ID, FCM_SA_KEY, EMAIL_PROVIDER, SENDGRID_API_KEY, EMAIL_FROM у .env.example + configuration-manual.md
- Оновити docs/spec.md розділом про lifecycle-канали ПЕРЕД кодом (rules §2)

**Поза scope:**
- WebSocket/real-time DM (окрема TASK-SOCIAL DM real-time)
- SMS/Viber/Telegram канали — тільки push+email у цій задачі
- Нові push/email фічі-тригери самі по собі (gift-delivery UX, event-reminder scheduling логіка) — вони споживачі каналу; тут лише транспорт+seam+існуючі тригери
- Web push (браузерний) — лише mobile FCM у цій ітерації
- Заміна payment-seam або escrow
- Повноцінний notification-preferences UI (є окрема ONBOARD/NOTIF задача) — тут лише поважати наявні opt-out (blocked, і per-channel mute якщо існує)
- In-app rich media в email (складні HTML-шаблони) — мінімальний коректний HTML+text

**Модель даних:**
- Міграція 0033_channel_retry.sql: ALTER TABLE notifications ADD COLUMN push_attempts INT NOT NULL DEFAULT 0, ADD COLUMN push_next_attempt_at TIMESTAMPTZ, ADD COLUMN push_last_error TEXT, ADD COLUMN push_failed_at TIMESTAMPTZ (dead-letter коли attempts>=MAX)
- Міграція 0033: ALTER TABLE orders ADD COLUMN email_attempts INT NOT NULL DEFAULT 0, ADD COLUMN email_next_attempt_at TIMESTAMPTZ, ADD COLUMN email_last_error TEXT (симетрично до наявного email_sent BOOLEAN з 0026)
- Розширення transactional email за межі order-confirmation: нова таблиця email_outbox(id UUID PK, kind TEXT CHECK(kind IN ('ticket','receipt','refund','guest_magic_code','order_confirmation')), to_email TEXT NOT NULL, subject TEXT, payload JSONB NOT NULL, sent_at TIMESTAMPTZ, attempts INT NOT NULL DEFAULT 0, next_attempt_at TIMESTAMPTZ, last_error TEXT, created_at TIMESTAMPTZ NOT NULL DEFAULT now()); наявний orders.email_sent-шлях лишається як є або мігрує в email_outbox рядок kind='order_confirmation' (обрати простіший: залишити orders.email_sent, а email_outbox лише для нових типів)
- device_tokens: без нових колонок; hard-fail FCM (UNREGISTERED) → DELETE рядка (наявна унікальність token зберігається)
- Індекси: notifications(pushed_at, push_next_attempt_at) WHERE pushed_at IS NULL; email_outbox(sent_at, next_attempt_at) WHERE sent_at IS NULL для ефективного drain
- БЕЗ змін у payment_settings/escrow; канальний provider — окремий env-конфіг, не в БД (простота)

**API:**
- GET /api/admin/channels/health (admin-guard) → 200 {push:{provider,configured:bool,sent_24h:i64,failed_24h:i64,pending:i64,dead_lettered:i64,oldest_pending_age_s:i64|null,last_error:string|null}, email:{...same shape...}}
- POST /api/admin/channels/test (admin-guard) → body {channel:'push'|'email', target:string} → 200 {delivered:bool, provider_ref:string|null, error:string|null}; push target=device_token, email target=email-адреса; НЕ пише в outbox, синхронний тест
- БЕЗ нових публічних (не-admin) endpoint-ів — транспорт працює всередині наявних push/email worker-ів; жоден бізнес-endpoint не змінює контракт (нема breaking API, rules §10)
- Внутрішній seam (не HTTP): trait EmailChannel + registry EmailChannel::from_env(); trait/функція fcm_send(access_token,project_id,token,msg)->Result<FcmOutcome{Delivered|Unregistered|Retryable(err)|Permanent(err)}>
- OAuth2 token-mint: внутрішня fn mint_fcm_access_token(sa_key)->Result<CachedToken{token,expires_at}> (RS256 JWT assertion до https://oauth2.googleapis.com/token), кеш у worker-стані

**UI:**
- Admin (окремий PR/наступна задача UI): картка 'Delivery health' на dashboard, що споживає GET /api/admin/channels/health — badge configured/not-configured, 24h sent/failed, oldest pending; кнопка 'Send test' → POST /api/admin/channels/test. У ЦІЙ задачі UI не обов'язковий (surface=API), лише endpoint-и; фронт може підхопити пізніше
- Android/Web attendee UI не змінюється — доставка прозора для клієнта

**Acceptance Criteria:**
- AC-1: Коли FCM_PROJECT_ID+FCM_SA_KEY задані, push_worker для кожного PendingPush робить реальний POST на fcm.googleapis.com/v1/projects/{id}/messages:send з валідним Bearer OAuth2-token; успіх (2xx) → notifications.pushed_at=now(); token НЕ помічається доставленим при помилці
- AC-2: OAuth2 access-token мінтується з SA-key (RS256 JWT assertion) і кешується; повторні send у межах ttl не роблять зайвого token-запиту; протухлий (>exp-60s) — оновлюється
- AC-3: FCM-відповідь UNREGISTERED/NOT_FOUND → рядок device_tokens видаляється, notification НЕ вважається fail для інших токенів того ж юзера (fan-out незалежний per-token)
- AC-4: Retryable-помилка (5xx/429/мережа) → push_attempts++, push_next_attempt_at=now()+backoff(attempts), pushed_at лишається NULL; після MAX_ATTEMPTS → push_failed_at=now() (dead-letter, більше не тягнеться в pending)
- AC-5: pending()-селект тягне лише notifications де pushed_at IS NULL AND push_failed_at IS NULL AND (push_next_attempt_at IS NULL OR push_next_attempt_at<=now())
- AC-6: EmailChannel::from_env() резолвить NoopEmail/LogEmail/SendGridEmail за EMAIL_PROVIDER; за замовчуванням (unset) → LogEmail (детермінований, CI-safe, реальних грошей/мережі нема)
- AC-7: email_worker для кожного PendingEmail викликає активний EmailChannel.send(); 2xx → sent watermark; помилка → email_attempts++/next_attempt_at/last_error, watermark НЕ ставиться
- AC-8: SendGridEmail робить POST api.sendgrid.com/v3/mail/send з Bearer SENDGRID_API_KEY, from=EMAIL_FROM, коректним to/subject/text+html; від build_confirmation та нових build_* (ticket/receipt/refund/guest_magic_code) шаблонів
- AC-9: Transactional-типи ticket, receipt, refund, guest_magic_code ставляться в email_outbox відповідними доменними подіями (mint, refund, guest checkout) і доставляються тим самим worker+seam
- AC-10: GET /api/admin/channels/health повертає точні 24г-лічильники sent/failed/pending/dead_lettered + provider + configured для обох каналів; неадмін → 403
- AC-11: POST /api/admin/channels/test {channel,email} синхронно шле тест через активний provider і повертає delivered/provider_ref/error без запису в outbox
- AC-12: Коли канал не сконфігурований (Noop/Log), продукт лишається повністю робочим — жоден шлях не 500-ить через відсутні creds (graceful degrade, як payment NoopProvider); health показує configured=false
- AC-13: .env.example + configuration-manual.md документують FCM_PROJECT_ID, FCM_SA_KEY, EMAIL_PROVIDER, SENDGRID_API_KEY, EMAIL_FROM; docs/spec.md оновлено ПЕРЕД кодом
- AC-14: guest magic-code email доставляється неавторизованому отримувачу (frictionless gift/guest-loop): claim-лінк у листі веде на claim без існуючого акаунта — розблоковує guest→registered activation KPI

**Security-вектори (SEC):**
- SEC-1: SA-key (FCM_SA_KEY) та SENDGRID_API_KEY — тільки з env/секрет-стору, НІКОЛИ не логуються, не потрапляють у health/test-response, не хардкодяться (rules §10); файл SA-key читається з path, не інлайниться
- SEC-2: OAuth2 access-token не логується і не повертається в API; кеш у пам'яті процесу, не в БД
- SEC-3: /api/admin/channels/* під admin-guard (як інші /api/admin/*); неавторизований/неадмін → 403; test-send rate-limited щоб не перетворити на open email/push-relay
- SEC-4: guest_magic_code токен у email — single-use, short-TTL, rate-limited per email/device (принцип 'frictionless не діряво'); лист не розкриває чужих PII; посилання підписане (signed-token seam)
- SEC-5: email до заблокованих (users.blocked) та opted-out не шлеться (наявний WHERE u.blocked=false зберегти; поважати per-channel mute якщо є)
- SEC-6: HTML-email екранує user-controlled поля (event_title, ім'я дарувальника, message) — no HTML-injection у листі
- SEC-7: FCM-payload не містить секретів/сирих QR-токенів (узгоджено з ETicketScreen QR-security задачею); push data мінімальний (deep-link id, не сам квиток)
- SEC-8: dead-letter/last_error зберігає message провайдера без credentials; помилки не відкривають внутрішню інфру назовні клієнту
- SEC-9: token-endpoint та FCM-хости — фіксований allowlist URL, без user-controlled хоста (SSRF-guard); reqwest без redirect-follow на довільні хости

**Test Matrix:**
- [POS] FCM 2xx → pushed_at виставлено, attempts не рушить
- [POS] OAuth2 token кешується: два send → один token-mint (мокнутий token-endpoint, лічильник викликів)
- [POS] EmailChannel::from_env → правильний impl за EMAIL_PROVIDER (noop/log/sendgrid); default→Log
- [POS] SendGrid 202 → email watermark; ticket/receipt/refund/guest_magic_code шаблони містять ключові поля
- [POS] guest_magic_code email побудований з валідним single-use claim-лінком
- [POS] GET /api/admin/channels/health повертає коректні 24г-лічильники (seed: 2 sent,1 failed,1 pending)
- [NEG] FCM 5xx → attempts++, next_attempt_at у майбутньому, pushed_at лишається NULL, не dead-letter доки < MAX
- [NEG] FCM UNREGISTERED → device_tokens рядок видалено, інші токени юзера доставлені
- [NEG] attempts>=MAX → push_failed_at виставлено, наступний pending()-drain НЕ повертає рядок
- [NEG] SendGrid 401/5xx → email не watermark, last_error записано, наступний tick ретраїть після backoff
- [NEG] channel не сконфігурований (Log) → worker не панікує, health.configured=false, продукт робочий
- [NEG] POST /api/admin/channels/test від неадміна → 403
- [SEC] SA-key/API-key відсутні в логах, health-response, test-response (grep-assert над captured output)
- [SEC] admin/channels/* без токена → 403; test-send за rate-limit → 429
- [SEC] guest magic-code: повторне використання токена → 410/консумовано; expired → 410; понад cap per email → rate-limited
- [SEC] HTML-email з event_title='<script>' → екрановано у відрендереному body
- [SEC] FCM data-payload не містить сирого qr_token (assert відсутності поля)
- [SEC] test-send target=внутрішній host/URL не приймається (SSRF allowlist)

---

## SDD-ANTIABUSE — Anti-abuse контур для guest/gift-claim (single-use short-TTL token + rate-limit + device/email-cap + magic-code)

**Мета:** Обгорнути весь frictionless guest/gift-claim шлях мінімальним, але повним anti-abuse контуром, який робить віральні фічі (gift-у-подарунок, guest-claim без акаунта) безпечними для увімкнення: одноразовий підписаний claim-token з коротким TTL (окремий секрет від QR/JWT), rate-limit і device/email-cap на free-claim, підтвердження власності через magic-code у листі (не лише лінк), позначка «квиток забрано з іншого пристрою» та auto-revert невибраного подарунка до дарувальника по TTL. Контур — блокуюча умова готовності (feature-flag OFF, поки не готовий), а не окрема пізніша задача. Провайдер-нейтральність збережена: жодна грошова логіка не додається — claim торкається лише unclaimed-квитків (free/RSVP на старті через NoopProvider).

**Поверхні:** API (Rust/Axum — новий api/src/claims.rs + міграції + reaper job у reminders/notifications loop), Web (public client: /claim/[token] лендінг + inline-signup, guest e-ticket /t/[token]), Android (Compose: екран claim з deep-link ug://claim/{token}, стани expired/wrong-code/cap), Admin (read-only: перегляд gift/claim-подій, ручний revert, скидання cap для support)  ·  **Effort:** M  ·  **Залежності:** INIT-TICKET-UNCLAIMED — вводить unclaimed-стан квитка (owner_id nullable/holder), без якого claim не має що привласнювати (hard dependency, координувати міграцію tickets), Наявний telemetry seam (api/src/telemetry.rs::log) — розширюється tracking-подіями, новий екран не потрібен, Email outbox seam (api/src/email.rs + 0026_email_outbox) — доставка magic-code листа, devices.rs / X-Device-Id — джерело device_hash для device-cap, Config/validate fail-fast патерн (config.rs) — для нового CLAIM_TOKEN_SECRET, Provider-agnostic payment seam (escrow.rs, NoopProvider) — контур не торкається грошей, лишається сумісним, Feature-flag інфраструктура (env-based) для GIFT_CLAIM_ENABLED, Web public client (/t/[token] guest e-ticket з INIT guest-checkout) для рендеру claim-лендінга

**У scope:**
- Окремий signed claim-token seam (api/src/claims.rs): HMAC-SHA256 над canonical payload {ticket_id, gift_id, exp, nonce}, секрет CLAIM_TOKEN_SECRET — ОКРЕМИЙ від JWT_SECRET і від QR-токена
- Single-use гарантія: claim атомарно споживає токен (nonce/gift-row переходить у claimed у тій самій транзакції, що й привласнення owner_id) — жодного повторного claim
- Short-TTL: за замовчуванням 72h (env CLAIM_TTL_HOURS), по exp токен невалідний; auto-revert невибраного подарунка назад до дарувальника фоновим reaper-джобом
- Magic-code підтвердження власності: 6-значний код у листі отримувачу (окремо від лінку); claim з guest-email вимагає code-match; лінк сам по собі не завершує claim для guest без акаунта
- Rate-limit на POST claim/reserve/gift-create: per-IP і per-email fixed-window лічильник у Postgres (без Redis — тримаємо просто)
- device/email-cap на FREE-claim: N free-claims на email за rolling-вікно і на device_hash за вікно (device_hash з наявного X-Device-Id / devices.rs); paid не капається
- Позначка «квиток забрано з іншого пристрою»: при claim фіксуємо claimed_device_hash; дарувальник і власник бачать факт claim + (маскований) пристрій
- Feature-flag GIFT_CLAIM_ENABLED (env, default false) — усі нові endpoint-и 404/403 поки OFF
- Tracking-події (розширення telemetry seam): gift_created, claim_link_opened, claim_code_sent, claim_succeeded, claim_failed{reason}, claim_reverted, ratelimit_blocked, cap_blocked
- Інтеграція з INIT-TICKET-UNCLAIMED: claim привласнює саме unclaimed-квиток (owner_id NULL / holder=gift), контур не мінтить нові квитки
- UX-стани на web /claim/[token] і Android: expired, already-claimed, cap-reached, wrong-code, need-signup (frictionless inline-signup у пару кліків)

**Поза scope:**
- Будь-яка грошова/escrow-логіка, реальний PSP, payout — не торкаємось (product-first, payment last)
- Ротація/зміна QR-токена на вході (окрема задача A.7 rotating QR) — claim-token це ІНШИЙ секрет і інший потік
- CAPTCHA / зовнішній anti-bot вендор — на старті достатньо rate-limit+cap+magic-code; лишаємо гачок, але не інтегруємо
- Distributed rate-limit сховище (Redis) — Postgres fixed-window достатньо для першого релізу
- Fraud-scoring ML / risk-engine — поза MVP контуру
- Phone/SMS-верифікація — тільки email magic-code на старті
- Зміна логіки paid-checkout gift-at-checkout понад привʼязку до цього claim-seam

**Модель даних:**
- migration 0033_gift_claims.sql: CREATE TABLE gift_claims (id UUID PK, ticket_id UUID NOT NULL REFERENCES tickets(id) ON DELETE CASCADE, sender_id UUID NOT NULL REFERENCES users(id), recipient_email CITEXT NULL, recipient_user_id UUID NULL REFERENCES users(id), nonce UUID NOT NULL, status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','claimed','reverted','expired')), magic_code_hash TEXT NULL, code_attempts SMALLINT NOT NULL DEFAULT 0, expires_at TIMESTAMPTZ NOT NULL, claimed_at TIMESTAMPTZ NULL, claimed_device_hash TEXT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT now())
- UNIQUE INDEX gift_claims_active_ticket_uniq ON gift_claims(ticket_id) WHERE status='pending' — один активний подарунок на квиток (single-use backstop)
- INDEX gift_claims_reaper_idx ON gift_claims(expires_at) WHERE status='pending' — для reaper
- migration 0034_claim_ratelimit.sql: CREATE TABLE claim_rate_events (id BIGSERIAL PK, bucket_key TEXT NOT NULL, window_start TIMESTAMPTZ NOT NULL, count INT NOT NULL DEFAULT 1); UNIQUE (bucket_key, window_start). bucket_key напр. 'ip:1.2.3.4:claim', 'email:a@b:freeclaim', 'device:<hash>:freeclaim'
- migration 0035_tickets_unclaimed.sql (координовано з INIT-TICKET-UNCLAIMED): ALTER TABLE tickets ALTER COLUMN owner_id DROP NOT NULL + додати holder статус; claim привласнює owner_id атомарно
- magic_code_hash: HMAC-SHA256(code, CLAIM_TOKEN_SECRET) — сирий код лише у листі, ніколи в БД/логах
- Config: додати claim_token_secret, claim_ttl_hours, gift_claim_enabled, free_claim_email_cap, free_claim_device_cap, claim_ratelimit_window; production validate() вимагає claim_token_secret != dev-default, len>=32, != jwt_secret

**API:**
- POST /api/tickets/:id/gift {recipient_email?, message?} -> 201 {claim_token, gift_id, expires_at} — створює gift_claims(pending), відправляє лист із лінком+magic-code (rate-limited, feature-flagged)
- GET /api/claims/:token -> 200 {event{title,starts_at,city,poster}, sender_name, status, needs_code:bool, needs_signup:bool} — публічний, БЕЗ auth; value-before-auth прев'ю; 410 gone якщо expired/claimed/reverted
- POST /api/claims/:token/send-code {email} -> 202 {} — (re)відправляє 6-значний magic-code на recipient_email; rate-limited per token+email
- POST /api/claims/:token/claim {code?, guest?{email,name}, device_id?} -> 200 {ticket{...}, jwt?} — атомарно: верифікує token(HMAC+exp)+nonce single-use+code-match+cap+rate-limit, привласнює owner_id (створює/логінить guest-акаунт при потребі), status->claimed, фіксує claimed_device_hash
- POST /api/events/:id/reserve — розширити: додати rate-limit+device/email-cap на free-claim; guest{email,name,device_id} шлях
- POST /api/admin/claims/:id/revert -> 200 — ручний revert до sender (support)
- POST /api/admin/rate-limit/reset {bucket_key} -> 200 — support скидання cap
- Усі claim-endpoint-и під GIFT_CLAIM_ENABLED; при OFF -> 404

**UI:**
- Web /claim/[token] (public, no auth): постер+назва+дата події, «Вам подарували квиток від {sender}», CTA «Забрати квиток»; стани: expired (410 → «Термін дії посилання минув»), already-claimed («Квиток уже забрано» + масковано пристрій/час), cap-reached, need-code (поле 6-значного коду + «Надіслати код повторно»), need-signup (inline: email+name → 1 клік → квиток+акаунт)
- Web e-ticket /t/[token] (guest): full-screen QR після успішного claim
- Android екран Claim (deep-link ug://claim/{token} та https-app-link): ті самі стани; при cap/wrong-code — зрозуміла помилка, не dead-end
- Android E-Ticket «Подарувати»: діалог email → створює gift, показує share-лінк для месенджерів (легкий шеринг), статус «очікує на отримувача / забрано»
- Мої квитки: сегмент «Подаровані» показує pending-gift зі статусом і кнопкою «Скасувати подарунок» (revert до TTL)
- Admin: read-only таблиця gift_claims (event, sender, recipient, status, expires_at) + дії Revert / Reset-cap

**Acceptance Criteria:**
- AC-1: Валідний claim-token із правильним magic-code для pending-подарунка привласнює квиток отримувачу, status->claimed, повертає e-ticket; gift-row single-use — повторний claim того ж token -> 410
- AC-2: claim-token підписаний ОКРЕМИМ CLAIM_TOKEN_SECRET (не JWT_SECRET, не QR): токен, підписаний JWT_SECRET, відхиляється; підробка payload (зміна ticket_id) -> 401/invalid
- AC-3: Прострочений claim-token (exp < now) -> 410 gone; reaper переводить pending gift у reverted після expires_at і повертає квиток дарувальнику (owner_id=sender)
- AC-4: Guest без акаунта завершує claim лише за наявності magic-code, надісланого на recipient_email; відкриття лінку без коду показує need-code, а не видає квиток (захист від крадіжки по відкритому першим лінку)
- AC-5: Free-claim (reserve+gift-claim) капається: понад free_claim_email_cap на email або free_claim_device_cap на device_hash у вікні -> 429 cap_blocked + tracking cap_blocked
- AC-6: Rate-limit per-IP і per-email на claim/send-code/gift: понад ліміт у вікні -> 429 ratelimit_blocked; лічильник у claim_rate_events атомарний (INSERT ... ON CONFLICT DO UPDATE ... RETURNING)
- AC-7: При claim фіксується claimed_device_hash; дарувальник і власник бачать «забрано з іншого пристрою» (масковано); повторна спроба з іншого пристрою на claimed-gift -> 410
- AC-8: GIFT_CLAIM_ENABLED=false → усі claim-endpoint-и повертають 404; фіча не активна, поки контур не увімкнено
- AC-9: magic-code: рівно 6 цифр, у БД лише HMAC-хеш; code_attempts обмежені (напр. 5) → блокування коду з вимогою «Надіслати код повторно»; сирий код ніколи не логується
- AC-10: Кожен крок емітить tracking-подію через telemetry seam (gift_created, claim_link_opened, claim_code_sent, claim_succeeded, claim_failed{reason}, claim_reverted, ratelimit_blocked, cap_blocked) — K-factor вимірюваний
- AC-11: Production Config.validate() падає, якщо CLAIM_TOKEN_SECRET = dev-default, коротший за 32 байти, або дорівнює JWT_SECRET
- AC-12: claim не мінтить нові квитки — лише привласнює існуючий unclaimed-квиток; sold-out подія не створює зайвих місць (seat-count незмінний при gift/claim/revert)

**Security-вектори (SEC):**
- SEC-1 Token brute-force: claim-token = ticket_id+gift_id+exp+128-bit nonce під HMAC-SHA256; перебір nonce обчислювально неможливий; невалідний токен -> 401 без витоку існування gift
- SEC-2 Replay / double-claim: single-use через UNIQUE(ticket_id) WHERE pending + атомарний CAS (UPDATE gift_claims SET status='claimed' WHERE id=$ AND status='pending' RETURNING) у тій самій транзакції, що owner_id — перегони дають рівно один claim
- SEC-3 Claim-after-sold-out / seat inflation: claim не викликає decrement_tickets і не мінтить; лише переносить owner_id існуючого квитка — місця не інфлейтяться
- SEC-4 Botnet free-farm: device/email-cap + per-IP rate-limit + magic-code-ownership спільно ламають масовий фарм free-квитків; cap_blocked/ratelimit_blocked трекаються для алертів
- SEC-5 Gift-theft по відкритому першим лінку: magic-code на recipient_email обов'язковий для guest — володіння лінком без пошти не завершує claim
- SEC-6 Secret-substitution / algorithm confusion: HMAC-SHA256 pinned, окремий секрет від JWT; токен під JWT_SECRET або з іншим алго відхиляється
- SEC-7 Magic-code brute: 6-значний код + ліміт code_attempts (5) + rate-limit на send-code; хеш у БД, constant-time порівняння
- SEC-8 Enumeration/timing: GET /api/claims/:token дає однакову відповідь-форму для invalid/expired/claimed (generic 410) щоб не розкривати валідність токена; send-code не підтверджує існування email
- SEC-9 Open-redirect / deep-link hijack: claim deep-link веде лише на канонічний /claim/{token}, token не використовується як redirect-URL; жодного user-controlled redirect
- SEC-10 PII/секрети в логах: recipient_email маскується, сирий magic-code і claim-token ніколи не логуються (тільки gift_id/nonce-hash у telemetry)
- SEC-11 Auto-revert race: reaper і ручний claim конкурують за той самий CAS на status; програвший отримує 410 — квиток не опиняється у двох власників

**Test Matrix:**
- [POS] Валідний token+code, pending gift -> claimed, e-ticket повертається, owner_id=recipient
- [POS] Guest без акаунта: token+code+guest{email,name} -> створено акаунт+jwt, квиток привласнено, signup у пару кліків
- [POS] Reaper: pending gift з expires_at<now -> reverted, owner_id повертається sender, seat-count незмінний
- [POS] Rate-limit лічильник: N запитів у вікні проходять, N+1 -> 429; наступне вікно скидає
- [NEG] Повторний claim того ж token -> 410 (single-use)
- [NEG] Прострочений token -> 410; підроблений payload -> 401
- [NEG] claim без code (guest) -> need-code, квиток НЕ виданий
- [NEG] Невірний code 5 разів -> code заблоковано, вимога re-send
- [NEG] Free-claim понад email-cap -> 429 cap_blocked; понад device-cap -> 429
- [NEG] GIFT_CLAIM_ENABLED=false -> усі endpoint-и 404
- [NEG] Gift на квиток, що вже має pending-gift -> 409 (UNIQUE активний)
- [SEC-1] Токен з випадковим nonce -> 401, без розкриття існування
- [SEC-2] Конкурентні claim того ж token (2 паралельні tx) -> рівно один claimed, інший 410
- [SEC-3] claim на sold-out подію -> success без інфляції місць (seat-count assert незмінний)
- [SEC-5] Token валідний, code відправлено на email отримувача, атакувальник із лінком без коду -> claim відхилено
- [SEC-6] Token підписаний JWT_SECRET/іншим алго -> 401
- [SEC-7] Brute magic-code понад ліміт -> блок; хеш у БД, не plaintext
- [SEC-8] invalid/expired/claimed token -> ідентична generic-відповідь (немає enumeration)
- [SEC-11] reaper vs claim гонка -> один переможець, квиток в одного власника
- [SEC] Config production з CLAIM_TOKEN_SECRET==JWT_SECRET або dev-default або <32B -> startup fail

---

## SDD-BROWSE-AUTH-001 — Browse-before-auth + passwordless email-код у claim/guest-контексті

**Мета:** Прибрати auth-wall із discovery: анонім бачить Афішу+деталі+«друзі йдуть» першим екраном; auth підіймається ЛИШЕ на дію (save/RSVP/buy/gift-claim). Анонімний intent персиститься і зливається в акаунт при першому логіні. Додати passwordless email-magic-code як ДРУГИЙ метод входу САМЕ у claim/guest-flow (Google-only лишається для повних акаунтів), з identity-merge unclaimed-білетів за email. Ціль — activation холодного трафіку, TTFW, claim/guest completion, K-factor ≥ 0.5. Тримати ПРОСТО: один signed-token seam, без другого Instagram.

**Поверхні:** API (Rust/Axum), Android (Compose), Web (Next.js), UX, MKT/tracking  ·  **Effort:** M  ·  **Залежності:** INIT-ROADMAP-SPEC (spec-виняток email-magic-code має бути внесений і апрувнутий засновником ПЕРЕД кодом — rules §0/§2/§3)., Email seam (api/src/email.rs + email-outbox 0026/0027) — вже є; розширити на login/claim-коди., Google Sign-In SDK на Android/Web (наразі Toast-заглушка AuthScreen.kt) — потрібен для /api/auth/google; блокує AC-3/AC-7 клієнтську частину., PaymentProvider seam (TASK-PROV-1, Noop/Test) — потрібен лише для guest PAID checkout; guest FREE/RSVP/claim не залежить від нього (можна вести незалежно)., 0020_email_ci (email CI unique) — вже є, потрібен для безпечного merge за email., Telemetry seam (telemetry.rs) — вже є; лише додати події воронки.

**У scope:**
- Навігація: анонім стартує на Афіші (не Onboarding); Onboarding відкладається до першої дії або показується non-blocking після першого логіну (Android AfishaScreen як start-destination; Web /home вже SSR-публічний).
- Auth-on-action gate: save/RSVP/buy/gift-claim показують bottom-sheet 'Continue with Google' (+ 'Отримати код на email' у guest/claim-контексті), без повного редіректу на /login.
- Анонімний intent persist: локальний device-scoped pending_intent {kind: save|rsvp|claim, event_id?, token?, state?} на клієнті; при першому успішному логіні POST /api/me/merge-intents дренує його серверно (idempotent).
- Passwordless email-code: нові endpoints request-code + verify-code; таблиця email_login_codes (6-значний код, TTL 10 хв, single-use, rate-limited, hashed). Видає app-JWT так само, як Google.
- Guest claim без акаунта: signed gift/guest token → сторінка перегляду квитка БЕЗ логіну (read); 'Забрати' → email-code inline → акаунт створюється тут же → білет прив'язується.
- Identity-merge: при першому Google-логіні (або email-code логіні) з email, що збігається з guest-email раніше отриманих unclaimed-білетів / guest-orders — авто-прив'язка (claim) цих квитків до акаунта в одній транзакції.
- Google_sub wiring: реалізувати POST /api/auth/google (id_token→app-JWT, upsert за email, заповнити users.google_sub) — передумова для merge за email.
- Guest checkout/reserve: reserve+checkout приймають опційний guest {email, name} коли викликач анонімний; free завжди; paid — лише якщо активний provider дозволяє (Noop/Test → так).
- Spec-виняток ПЕРЕД кодом: зафіксувати у docs/spec.md дозвіл email-magic-code ЛИШЕ у claim/guest-flow як другий метод (потребує апруву засновника) — блокує старт коду.
- Tracking-події для всієї воронки (browse→gate→login→claim→re-share) через наявний telemetry seam.

**Поза scope:**
- Email/password логін як загальний метод (лишається/прибирається окремою TASK-AUTH-1; тут НЕ розширюємо його).
- SMS/phone auth, соц-логіни крім Google.
- Refresh-токени / ротація сесій.
- Rotating QR (TASK-QR-1), ticket tiers (TASK-TIERS-1), реальний PSP (Фаза 4) — окремі задачі; тут лише інтеграційні точки.
- Повний онбординг-редизайн (DOB-крок тощо — TASK-ONBOARD-1); тут лише робимо його non-blocking.
- Real-time DM, композер, медіа — поза scope.

**Модель даних:**
- Міграція 0033_email_login_codes.sql: email_login_codes(id UUID PK, email TEXT NOT NULL, code_hash TEXT NOT NULL, purpose TEXT NOT NULL CHECK(purpose IN ('login','claim')), consumed_at TIMESTAMPTZ, expires_at TIMESTAMPTZ NOT NULL, attempts INT NOT NULL DEFAULT 0, created_ip INET, created_at TIMESTAMPTZ NOT NULL DEFAULT now()); INDEX(email, purpose, created_at DESC); код зберігається лише як хеш (SHA-256/argon2), НІКОЛИ plaintext.
- Міграція 0034_guest_orders.sql: додати orders.guest_email TEXT (nullable), orders.claim_token TEXT UNIQUE (nullable, single-use signed ref); tickets.claimed_at TIMESTAMPTZ; buyer_id стає nullable ДЛЯ guest-orders АБО (простіший варіант, обраний) — buyer_id вказує на lazily-створеного placeholder-user з email і claimed=false прапорцем: users.claimed BOOLEAN NOT NULL DEFAULT true, guest-user → claimed=false, без google_sub/пароля.
- users: використати наявні google_sub (0001) — заповнюється в /api/auth/google; email вже UNIQUE та CI (0020_email_ci). Додати users.claimed (див. вище) для розрізнення guest-placeholder від повного акаунта.
- БЕЗ схемних змін для pending_intent — це КЛІЄНТСЬКИЙ стан (Android DataStore / web localStorage); сервер лише отримує його в /api/me/merge-intents і застосовує ідемпотентно.
- Rate-limit стан для request-code — з наявного патерну (лічильник у email_login_codes за (email, вікно) + опційно per-IP); без нової інфри.

**API:**
- POST /api/auth/google {id_token} → 200 {token, user}: верифікує Google id_token (aud=GOOGLE_CLIENT_ID, iss, exp), upsert user за email, заповнює google_sub; якщо існував guest-placeholder з тим email → promote (claimed=true) + claim unclaimed tickets. Невалідний token → 401.
- POST /api/auth/email-code/request {email, purpose: login|claim} → 204: генерує 6-значний код, зберігає hash + expires_at(now+10хв), надсилає через email seam. Rate-limit: ≤5 запитів/email/год і ≤N/IP; завжди 204 (не розкривати існування email). Container-first: код у dev видимий у email-outbox.
- POST /api/auth/email-code/verify {email, code, purpose} → 200 {token, user}: single-use, звіряє hash, expires_at, attempts<5; consume; login→видає JWT (upsert-user за email, claimed=true); claim→те саме + прив'язує unclaimed tickets за email. Невірний/прострочений/вичерпаний → 401; забагато спроб → 429.
- POST /api/me/merge-intents {intents: [{kind, event_id?, claim_token?, state?}]} (Bearer) → 200 {applied: [...], skipped: [...]}: серверно застосовує анонімні дії (save/rsvp/claim) до акаунта; ідемпотентно (повторний виклик не дублює); claim валідує claim_token single-use.
- GET /api/tickets/claim/:claim_token → 200 {ticket_public}: PUBLIC (без auth) read-only перегляд подарованого квитка (event title/date/city/poster, статус claimed/unclaimed, recipient_masked). Невалідний/consumed → 404.
- POST /api/tickets/claim/:claim_token {via: google|email} (Bearer ПІСЛЯ inline-auth) → 200 {ticket}: прив'язує білет до автентифікованого акаунта; single-use FOR UPDATE; вже claimed → 409.
- POST /api/events/:id/reserve — розширити: приймає опційний {guest:{email,name}} коли викликач анонімний → створює guest-placeholder user (claimed=false) + mint + claim_token; повертає {ticket, claim_token}. Автентифікований шлях незмінний.
- POST /api/events/:id/checkout — розширити: {..., guest:{email,name}?} для анонімного paid (лише якщо provider дозволяє; Noop/Test → так), інакше 401 з provider_action hint. RSVP: POST /api/events/:id/rsvp {state} лишається auth-only, але gate спрацьовує через bottom-sheet, не редірект.

**UI:**
- Android AfishaScreen — START destination застосунку для аноніма (замість форсованого Onboarding→Auth). Показує trending (GET /api/discover без токена вже підтримано) + 'друзі йдуть' коли залогінений. Onboarding → non-blocking, доступний після першого логіну.
- AuthBottomSheet (Android+Web) — уніфікований gate на дію: заголовок за контекстом ('Увійдіть, щоб зберегти/піти/купити/забрати'), кнопка 'Continue with Google'; у guest/claim-контексті ДОДАТКОВО 'Отримати код на email' → EmailCodeSheet.
- EmailCodeSheet — крок 1 email → request-code; крок 2 6-значний код (auto-advance, resend-таймер 30с, помилки 401/429). Успіх → JWT збережено → застосовано pending_intent.
- Guest claim page (Web /t/claim/[token] + Android deep-link underground://ticket/claim/:token) — публічний перегляд квитка (постер/подія/статус) БЕЗ логіну; CTA 'Забрати квиток' → EmailCodeSheet (purpose=claim) → акаунт+claim у пару кліків.
- Deep-link share: кнопка 'Поділитися' на Деталях події й на E-Ticket-подарунку генерує лінк (event: underground://event/:id + web /events/:id; gift: /t/claim/:token) для месенджерів.
- Pending-intent UX: після логіну автоматично довершується перервана дія (напр. save застосовано, показано toast 'Збережено'), без повторного тапу.

**Acceptance Criteria:**
- AC-1: Анонім (без токена) відкриває застосунок → бачить Афішу з trending одразу; жоден екран не редіректить його на Onboarding/Auth до першої дії.
- AC-2: Анонім відкриває Деталі події й соц-блок 'друзі йдуть' (лічильники) без логіну; tap на save/RSVP/buy показує AuthBottomSheet, а не повний /login-редірект.
- AC-3: POST /api/auth/google з валідним id_token створює/знаходить user за email, заповнює google_sub, видає JWT; повторний логін тим же Google — той самий user (без дубля).
- AC-4: request-code → на email (в dev — outbox) приходить 6-значний код; verify-code з правильним кодом у межах 10 хв видає валідний JWT і user; неправильний/прострочений → 401.
- AC-5: Guest без акаунта отримує безкоштовний квиток через reserve{guest} → повертається claim_token; перегляд GET /api/tickets/claim/:token працює БЕЗ auth.
- AC-6: Guest 'Забирає' квиток email-кодом (purpose=claim) → створюється/знаходиться акаунт за email → білет прив'язується (claimed=true), claim_token стає consumed (повторний claim → 409).
- AC-7: Identity-merge: user отримав guest-квиток на email X, потім уперше логіниться Google/email-кодом з тим самим email X → усі unclaimed-квитки з guest_email=X авто-прив'язуються до акаунта в одній транзакції.
- AC-8: pending_intent: анонім тапнув save → пройшов auth → save застосовано автоматично (POST /api/me/merge-intents), без повторного тапу; повторний дренаж того ж intent нічого не дублює (ідемпотентність).
- AC-9: Spec-виняток email-magic-code (лише claim/guest-flow, другий метод) присутній у docs/spec.md ДО мерджу коду, з позначкою founder-approval.
- AC-10: Share-лінк події та gift-claim-лінк відкриваються на web та через Android deep-link; для аноніма ведуть на публічний перегляд, не на login-wall.
- AC-11: Уся воронка (event_view_anon, auth_gate_shown, login_success, guest_claim_started, guest_claim_completed, intent_merged, share_clicked) пише telemetry-події через наявний seam.

**Security-вектори (SEC):**
- SEC-1: email_login_codes зберігає лише hash коду (SHA-256/argon2), НІКОЛИ plaintext; код одноразовий (consumed_at), TTL ≤10 хв, attempts<5 інакше 429.
- SEC-2: request-code rate-limited (≤5/email/год, per-IP cap) і завжди повертає 204 незалежно від існування email (без user-enumeration через статус чи тайминг).
- SEC-3: claim_token — signed single-use short-TTL; consume під FOR UPDATE у транзакції; повторний/паралельний claim → 409/404, білет не дублюється (anti-double-claim).
- SEC-4: Google id_token верифікується повністю (підпис через Google JWKS, aud=GOOGLE_CLIENT_ID, iss ∈ {accounts.google.com, https://accounts.google.com}, exp); підроблений/чужий-aud → 401.
- SEC-5: identity-merge прив'язує лише квитки, чий guest_email СУВОРО дорівнює верифікованому email логіну (CI-порівняння через 0020_email_ci); email вважається верифікованим ЛИШЕ через Google email_verified=true або успішний email-code того ж email — жодного merge за непідтвердженим вводом.
- SEC-6: email-magic-code дозволений ЛИШЕ для purpose∈{login (guest-context), claim}; не стає бекдором до повних Google-акаунтів понад доктрину (повний акаунт з google_sub може логінитись кодом лише на той самий email — не ескалює прав).
- SEC-7: /api/me/merge-intents застосовує лише дії від імені caller; claim у intent валідує claim_token (не можна claim'нути чужий білет підставивши event_id); age-gate і published-перевірки reserve/checkout зберігаються для guest-шляху.
- SEC-8: pending_intent — device-scoped, не приймати серверно нічого, що обходить authorization (наприклад intent з чужим ticket_id без валідного token → skip).
- SEC-9: guest-placeholder (claimed=false) не має пароля/сесії доти, доки не claimed; не може логінитись інакше ніж через verified email того ж рядка; blocked-перевірка застосовується.
- SEC-10: PII у публічному GET /api/tickets/claim/:token — recipient email маскується; постер/подія публічні, але owner-контакти ні.

**Test Matrix:**
- [POS] AC-1 integration: GET /api/discover без Authorization → 200 trending; Android UI-тест: start destination = Afisha для порожнього TokenStore.
- [POS] AC-3 integration: /api/auth/google (замоканий verifier) валідний → 200, google_sub заповнено; повторний → той самий user_id.
- [POS] AC-4 integration: request→verify happy path видає JWT; unit: генерація/hash/expiry коду.
- [POS] AC-5/AC-6 integration: reserve{guest}→claim_token→public GET→email-code claim→ticket.owner=акаунт, claimed=true.
- [POS] AC-7 integration: guest-квиток(email X) + пізніший google-login(email X, verified) → квиток авто-прив'язано в одній tx.
- [POS] AC-8 integration: merge-intents {save} застосовує save; повторний виклик — без дубля (idempotent).
- [NEG] verify-code неправильний код → 401; прострочений (>10хв) → 401; 6-та спроба → 429.
- [NEG] claim вже claimed токен → 409; невалідний/consumed token у public GET → 404.
- [NEG] checkout guest paid коли provider не дозволяє → 401 з provider_action hint; reserve guest на past/age-gated подію → 409 (гейти збережено).
- [NEG] merge-intents з intent, що містить чужий ticket_id без валідного claim_token → skip (не застосовано).
- [SEC] SEC-1: у БД немає plaintext-коду; consumed код повторно → 401.
- [SEC] SEC-2: request-code для неіснуючого email → 204 (немає enumeration); >5/год → rate-limited; тайминг-інваріантність.
- [SEC] SEC-3: два паралельні claim одного token → рівно один 200, інший 409; білет не подвоєно (concurrency-тест).
- [SEC] SEC-4: google id_token з чужим aud / поганим підписом / простроч. → 401.
- [SEC] SEC-5: merge НЕ спрацьовує коли guest_email≠login email або email не verified.
- [SEC] SEC-6: повний Google-акаунт email-код-логіном не отримує розширених прав; purpose поза {login,claim} → 400/401.
- [SEC] SEC-10: public claim GET не віддає повний recipient email (маскування).

---

## SDD-ONB-1 — Не-блокуючий онбординг ≤1 екран + contextual permission priming + lazy 18+ age-gate

**Мета:** Скоротити time-to-first-event: обов'язковий онбординг = 1 екран (категорії + місто, дефолт Київ), який можна пропустити; DOB виведений з онбордингу в точковий age-gate на 18+ checkout; push/гео питаються через pre-permission priming (push — після першого RSVP/save, гео — при тапі «Поблизу»), а не сирим OS-промптом на холодному юзері. Ввести TTFW як телеметричну метрику. Ціль — зростання onboarding completion, activation та push opt-in без нових бекенд-таблиць (reuse наявних endpoint-ів).

**Поверхні:** Android, API (reuse, no breaking change), Telemetry  ·  **Effort:** S  ·  **Залежності:** INIT-BROWSE-AUTH (робочий Google Sign-In + перегляд Афіші до/після auth) — онбординг тригериться після auth і веде на Афішу, Наявні endpoint-и: prefs.rs (PUT /api/me/prefs), auth.rs (PATCH /api/me), devices.rs (POST /api/me/devices), telemetry.rs (POST /api/telemetry) — усі присутні в routes.rs, змін не потребують, orders.rs meets_age/age_rating 409-контракт — клієнтська обробка залежить від нього

**У scope:**
- Android: залишити ОБОВ'ЯЗКОВИМ рівно 1 екран онбордингу — InterestsEditor (категорії multi-chip + місто, дефолт «Київ») з видимою кнопкою «Пропустити»; жодних додаткових обов'язкових кроків (DOB/гео/follow/push) ПЕРЕД першим переглядом Афіші
- Android: прибрати DOB з онбордингу як окремий крок; DOB збирається ленивим age-gate лише в момент checkout на подію з age_rating>=18 (inline-діалог з date picker → PATCH /api/me {dob} → повтор checkout), а не для 100% юзерів
- Android: pre-permission priming для push — м'який in-app екран/bottom-sheet з поясненням цінності ПЕРЕД системним промптом, тригериться після першого успішного RSVP (going/interested) або save; лише при згоді викликається OS-промпт і POST /api/me/devices {token}
- Android: pre-permission priming для геолокації — softprompt тригериться при тапі сегмента/фільтра «Поблизу»/«Біля тебе»; лише при згоді — OS fine-location промпт; при відмові фільтр деградує до вибору міста без падіння
- Telemetry: інструментувати воронку через наявний POST /api/telemetry {name, props} — події onboarding_shown, onboarding_completed, onboarding_skipped, permission_prime_shown/accepted/denied (channel=push|geo), first_valuable_action (rsvp|save|ticket); вивести TTFW = t(first_valuable_action) − t(signup) як обчислювану метрику в telemetry-агрегації
- Android: інтерпретувати 409 age-gate від checkout не як глухий фейл, а як тригер inline DOB-діалогу з подальшим ретраєм
- Reuse наявних endpoint-ів без breaking-змін: PUT /api/me/prefs, PATCH /api/me, POST /api/me/devices, POST /api/telemetry

**Поза scope:**
- Нові бекенд-таблиці або міграції (усі потрібні endpoint-и вже існують)
- follow-suggestions крок у онбордингу (контекстно пізніше, не в цій задачі)
- Зміна логіки meets_age / age_rating семантики в orders.rs (лишається як є; змінюється лише клієнтська обробка 409)
- Web-онбординг (окрема задача; ця SDD — Android+telemetry)
- Реальна платіжка/PSP (product-first, NoopProvider)
- Повноцінний геофенсинг/фонова геолокація — тільки one-shot fine-location за запитом «Поблизу»
- Push-transport (FCM) зміни — reuse наявного devices/push seam

**Модель даних:**
- БЕЗ нових таблиць/міграцій. Reuse: user_prefs(user_id, categories, city, country, locale, onboarded-derived, updated_at) — PUT /api/me/prefs
- Reuse: users.dob (NaiveDate, nullable) — PATCH /api/me {dob}; валідація future-date вже в auth.rs:219
- Reuse: devices(user_id, token, ...) — POST /api/me/devices
- Reuse: telemetry_events(user_id nullable, name, props JSONB, created_at) — INSERT через telemetry::log; нові name-значення лише строкові константи, схема не змінюється
- Reuse: events.age_rating ('13'|'16'|'18'|'21') — визначає, чи потрібен age-gate на checkout

**API:**
- PUT /api/me/prefs (reuse) — req {categories: string[<=8], city: string, near_me?: bool, radius_km?: int}, resp {categories, city, onboarded:true}; near_me/radius_km — опційні, ігноруються бекендом якщо не підтримані (non-breaking)
- PATCH /api/me (reuse) — req {dob: 'YYYY-MM-DD'}, resp UserDto; 400 при future-date (наявна валідація)
- POST /api/me/devices (reuse) — req {token: string}, resp 204; викликається ЛИШЕ після priming-згоди + OS-grant
- POST /api/telemetry (reuse, auth-optional) — req {name: string(<=64), props: JSON}; нові name: 'onboarding_shown'|'onboarding_completed'|'onboarding_skipped'|'permission_prime_shown'|'permission_prime_accepted'|'permission_prime_denied'|'first_valuable_action'
- POST /api/orders/checkout (reuse) — при 409 (age-gate, meets_age=false) клієнт показує DOB-діалог; після PATCH /api/me {dob} — ретрай того ж checkout

**UI:**
- OnboardingScreen (Android, наявний): рівно 1 екран — заголовок «Що тобі цікаво?», FlowRow категорій (multi-chip), CityField (дефолт «Київ»), кнопка «Зберегти (N)» + TextButton «Пропустити»; НЕ додавати DOB/push/geo кроків
- AgeGateDialog (Android, новий): inline bottom-sheet/dialog з date picker «Підтверди дату народження — подія 18+»; тригер — 409 на checkout; save → PATCH /api/me {dob}; при <18 — зрозуміле повідомлення без глухого фейлу; успіх → авто-ретрай checkout
- PushPrimingSheet (Android, новий): bottom-sheet «Нагадаємо, коли друзі підуть / подія починається» з CTA «Увімкнути» / «Не зараз»; тригер — після 1-го RSVP/save; «Увімкнути» → OS-промпт → POST /api/me/devices
- GeoPrimingSheet (Android, новий): bottom-sheet «Покажемо події поруч» при тапі «Поблизу»; CTA «Дозволити» → OS fine-location; відмова → fallback на місто з prefs без помилки

**Acceptance Criteria:**
- AC-1: Новий юзер після signup бачить РІВНО 1 обов'язковий екран онбордингу (категорії+місто); після «Зберегти» або «Пропустити» одразу потрапляє на Афішу — жодних DOB/push/geo екранів між ними
- AC-2: «Пропустити» на онбордингу веде на Афішу і НЕ блокує подальше користування; prefs позначаються onboarded (наявна поведінка через PUT з порожнім вибором)
- AC-3: DOB НЕ питається під час онбордингу за жодних умов
- AC-4: При checkout на подію з age_rating>=18 у юзера без DOB checkout повертає 409, і клієнт показує AgeGateDialog замість глухої помилки; після введення валідного DOB (вік>=age_rating) checkout успішно ретраїться і минтить квиток
- AC-5: При DOB, що дає вік < age_rating, показується зрозуміле повідомлення (без крешу/тихого 409), квиток не минтиться
- AC-6: Push OS-промпт НЕ показується під час онбордингу; PushPrimingSheet з'являється лише ПІСЛЯ першого успішного RSVP або save, і OS-промпт викликається тільки якщо юзер тапнув «Увімкнути»
- AC-7: POST /api/me/devices викликається виключно після grant OS-дозволу; відмова від priming або OS-промпту не викликає devices-endpoint і не блокує апп
- AC-8: Гео OS-промпт викликається лише після тапу «Поблизу» + згоди в GeoPrimingSheet; відмова деградує до фільтра за містом без помилки
- AC-9: Кожна ключова точка воронки шле telemetry-подію (onboarding_shown/completed/skipped, permission_prime_* з props.channel, first_valuable_action з props.action); telemetry-фейл ніколи не блокує UI-дію (best-effort, як у telemetry.rs)
- AC-10: first_valuable_action логується один раз на юзера при першій із дій rsvp|save|ticket; TTFW обчислюється в telemetry-агрегації як різниця з signup
- AC-11: Жодних breaking-змін до наявних API; PUT /api/me/prefs приймає опційні near_me/radius_km без 4xx якщо бекенд їх ще не читає

**Security-вектори (SEC):**
- SEC-1: Age-gate — DOB валідується серверно (meets_age поверх users.dob); клієнт НЕ може обійти 18+ мінт, підмінивши локальний прапорець; єдине джерело істини — 409 від checkout, ретрай теж проходить серверну перевірку
- SEC-2: DOB — PII; PATCH /api/me {dob} лише для власного акаунта (CurrentUser), не логується в telemetry props у сирому вигляді (телеметрія несе лише факт події, не дату народження)
- SEC-3: Future-dated або невалідний DOB відхиляється сервером (наявна валідація auth.rs:219) → 400; клієнт показує помилку, не мовчить
- SEC-4: Push-токен реєструється лише після явного OS-grant; жодної реєстрації devices на холодному/відмовленому юзері (уникнення привласнення токена/spam-каналу)
- SEC-5: Геолокація — one-shot за явним user-intent («Поблизу») + OS-grant; жодного фонового збору координат; відмова не деградує безпеку
- SEC-6: Telemetry POST auth-optional і best-effort — не приймає й не персистить PII (DOB/точні координати/токени); rate/size-cap на name(<=64) наявний; props не містять секретів
- SEC-7: onboarding-skip не підвищує привілеї й не обходить жодних гейтів (age-gate спрацює пізніше на checkout)

**Test Matrix:**
- [POS] Signup → бачить 1 екран → «Зберегти» з категоріями+містом → PUT /api/me/prefs 200, onboarded=true → Афіша
- [POS] Signup → «Пропустити» → Афіша, подальші дії не блокуються
- [POS] Checkout на 18+ подію з валідним DOB (вік>=rating) → 200, квиток мінтиться
- [POS] Перший RSVP → PushPrimingSheet показано → «Увімкнути» → OS-grant → POST /api/me/devices 204
- [POS] Тап «Поблизу» → GeoPrimingSheet → «Дозволити» → grant → події за радіусом
- [POS] first_valuable_action логується рівно один раз при першому save
- [NEG] Checkout на 18+ без DOB → 409 → AgeGateDialog (не креш, не тихий фейл)
- [NEG] DOB дає вік < age_rating → зрозуміле повідомлення, квиток не мінтиться
- [NEG] Відмова від push priming → devices-endpoint НЕ викликається, апп працює
- [NEG] Відмова від гео OS-промпту → fallback на місто, без помилки
- [NEG] Telemetry endpoint 5xx/timeout → UI-дія (RSVP/checkout/onboarding) завершується успішно (best-effort)
- [NEG] PUT /api/me/prefs з >8 категоріями → 400 (наявний MAX_CATEGORIES), клієнт показує помилку
- [SEC] Клієнт з підробленим локальним 'age_ok' прапорцем → сервер все одно 409 на 18+ без DOB
- [SEC] PATCH /api/me {dob} чужого юзера → 401/403 (CurrentUser scope)
- [SEC] Future-dated DOB → 400
- [SEC] devices-register без OS-grant (симуляція прямого виклику) не досяжний зі стандартного flow; priming-denied шлях не реєструє токен
- [SEC] Telemetry props з DOB/координатами — не відправляються клієнтом (перевірка payload у тестах інструментації)
- [SEC] onboarding-skip не надає доступу до 18+ мінту в обхід age-gate

---

## SDD-020 — Tracking Plan v1 + purchase-воронка + K-factor атрибуція + North-Star дашборд

**Мета:** Зробити зростання Underground ВИМІРЮВАНИМ як контракт: розширити наявний telemetry-seam (`api/src/telemetry.rs`, таблиця `telemetry_events`, `POST /api/telemetry`) до канонічного Tracking Plan v1 (~20 подій з фіксованою property-схемою й точками емісії), server-side purchase-воронки (view→detail→checkout_start→paid→attended з drop%), парної атрибуції віральних loops (share_token/referrer на gift/invite/share, *_sent↔*_accepted) і admin-дашборду North-Star (tickets/active-user 28d), funnel, timeseries (MAU/D7/D30), K-factor. Кожна віральна/конверсійна фіча роадмапу отримує вимірювальний контракт ПЕРЕД релізом. Рішення — дешеве розширення seam, БЕЗ нових user-facing екранів у застосунку, provider-agnostic (працює на free/RSVP + NoopProvider).

**Поверхні:** API, Admin, CROSS (Android + Web клієнти емітять client-side події за Tracking Plan)  ·  **Effort:** M  ·  **Залежності:** Наявний telemetry-seam: `api/src/telemetry.rs`, таблиця `telemetry_events` (міграція 0016), `POST /api/telemetry`, `GET /api/admin/telemetry` — розширюються, не переписуються., Auth-seam `CurrentUser`/`CurrentAdmin` (api/src/auth.rs) для опційної auth і admin-gate., Signed short-TTL single-use token-seam (наявний gift-transfer / `tokens_valid_after`, міграція 0031) — джерело для resolve referrer_user_id; K-factor парність залежить від того, що share/gift/invite фічі емітять *_sent при створенні токена., Rate-limit middleware (має існувати або додати мінімальний per-IP/per-user ліміт на POST /api/telemetry) для SEC-5., Admin app shell (`admin/`) + наявний auth поверх `/api/admin/*` для сторінки Analytics., docs/spec.md — зафіксувати визначення North-Star, funnel-контракт і Tracking Plan як істину ПЕРЕД кодом (rules §0/§2); ця задача тягне spec-зміну першою., Provider-agnostic payment seam (NoopProvider) — checkout_* мають працювати без реального провайдера.

**У scope:**
- Tracking Plan v1: канонічний реєстр ~20 подій з іменем, обов'язковими/опційними property, типом, point-of-emit; кодифікований як Rust-enum `TelemetryEvent` + JSON-довідник `docs/analytics/tracking-plan-v1.md` (єдине джерело істини для клієнтів і server-side).
- Server-side емісія термінальних/довіркових подій там, де стан вже змінюється на сервері: checkout_started, checkout_paid, checkout_failed, rsvp, gift_sent, gift_claimed, invite_accepted, scan_result (розширити `validate`), signup (вже є), ticket(free) (вже є).
- Client-emitted події через існуючий `POST /api/telemetry` з валідацією за схемою: event_view, tier_select, share_clicked, invite_sent, gift_claim_viewed, onboarding_step, app_open, session_start.
- Валідація property-схеми на вході `POST /api/telemetry`: невідоме ім'я події → 400; відсутні required-property → 400; зайві property дропаються (не зберігаються).
- Віральна атрибуція: колонки `share_token` (short-TTL single-use, уже є token-seam для gift) + `referrer_user_id` у payload віральних подій; server проставляє `referrer_user_id` з розв'язаного share_token при claim/accept, формуючи пару *_sent (source_user) ↔ *_accepted (referrer_user_id).
- Purchase-воронка (server-side, admin): `GET /api/admin/telemetry/funnel?from&to&event_id?` → впорядковані сходинки [event_view, tier_select, checkout_started, checkout_paid, attended] з absolute count + step_conversion% + drop%.
- Timeseries: `GET /api/admin/telemetry/timeseries?metric&bucket=day|week&from&to` для метрик active_users(app_open/session), new_signups, tickets_issued; похідні MAU / D7 / D30 retention рахуються з app_open/session_start подій.
- North-Star + K-factor: `GET /api/admin/telemetry/northstar?window_days=28` → tickets_per_active_user (paid+free/RSVP), k_factor = invites_accepted / active_users, share→claim%, guest→signup%; кожна ціль релізу як бінарний pruning-прапор (met/not_met) проти порогів (share→claim ≥30%, guest→signup ≥40%, K ≥0.5).
- Admin-UI (`admin/`): одна сторінка `Analytics` — North-Star картки, funnel-бар, timeseries-графік, K-factor + pruning-світлофор; поверх нових `/api/admin/telemetry/*`.
- Індекси й агрегації, що тримають запити дешевими на масштабі (partial/expression index на name+created_at; вже є `telemetry_events_name_idx`).

**Поза scope:**
- Реалізація самих gift/share/guest/invite фіч (це окремі INIT-и) — тут ЛИШЕ вимірювальний контракт і колонки атрибуції; емісія підключається як дешевий hook у тих задачах.
- Реальний платіжний провайдер/ledger — checkout_* події працюють на NoopProvider (product-first, payment-last).
- BI/warehouse-експорт, зовнішня аналітика (Amplitude/GA), cohort-explorer, ML-атрибуція мультитач — v2.
- PII у telemetry props (email/імена) — заборонено; лише id/enum/minor-units.
- Клієнтський аналітичний SDK/буферизація/офлайн-черга — застосунок емітить прямими POST best-effort.
- Зміна North-Star визначення чи escrow payout-latency SLA (supply контр-метрика вимірюється окремо в payout-задачі).

**Модель даних:**
- Розширити наявну `telemetry_events(id, user_id NULL, name TEXT, props JSONB, created_at)` — БЕЗ breaking-міграції; додати колонки атрибуції для швидких join-ів воронки.
- Міграція `0033_telemetry_v1.sql`: ALTER TABLE telemetry_events ADD COLUMN event_id UUID NULL REFERENCES events(id) ON DELETE SET NULL; ADD COLUMN session_id UUID NULL; ADD COLUMN referrer_user_id UUID NULL REFERENCES users(id) ON DELETE SET NULL; ADD COLUMN amount_minor BIGINT NULL; ADD COLUMN is_guest BOOLEAN NOT NULL DEFAULT false. (Promoted з props у типізовані колонки для funnel/атрибуції; решта property лишається в props JSONB.)
- Індекси: CREATE INDEX telemetry_events_event_idx ON telemetry_events(event_id, name, created_at); CREATE INDEX telemetry_events_ref_idx ON telemetry_events(referrer_user_id) WHERE referrer_user_id IS NOT NULL; CREATE INDEX telemetry_events_session_idx ON telemetry_events(session_id, created_at) WHERE session_id IS NOT NULL.
- БЕЗ окремої таблиці подій-реєстру: канон живе в коді (`TelemetryEvent` enum + schema-map) і в `docs/analytics/tracking-plan-v1.md`; runtime-валідація читає code-map.
- Share/gift токен-атрибуція спирається на наявний signed short-TTL token-seam (той, що вже покриває gift-transfer/`tokens_valid_after`); `share_token` не зберігається в telemetry — резолвиться в `referrer_user_id` на сервері при claim/accept.
- props-конвенція (canonical): гроші — ЗАВЖДИ minor-units (`amount_minor` BIGINT), валюта UAH за замовчуванням; жодних float/PII.

**API:**
- POST /api/telemetry — розширити наявний `log_event`: валідація за Tracking Plan (name ∈ enum, required props присутні, типи збігаються); promote відомих полів (event_id, amount_minor, is_guest, session_id) у колонки; 400 на невідоме ім'я/відсутній required; 204 як зараз. Auth опційна (guest events → user_id NULL).
- GET /api/admin/telemetry — лишити (backward-compat, 7d counts).
- GET /api/admin/telemetry/funnel?from=<date>&to=<date>&event_id=<uuid?> — CurrentAdmin; повертає [{step, count, step_conversion_pct, drop_pct}] для сходинок [event_view, tier_select, checkout_started, checkout_paid, attended]; attended = scan_result{result:'ok'}.
- GET /api/admin/telemetry/timeseries?metric=active_users|new_signups|tickets_issued&bucket=day|week&from&to — CurrentAdmin; [{bucket_start, value}].
- GET /api/admin/telemetry/northstar?window_days=28 — CurrentAdmin; {tickets_per_active_user, active_users, tickets_issued, k_factor, invites_accepted, share_to_claim_pct, guest_to_signup_pct, retention_d7_pct, retention_d30_pct, goals:[{name, threshold, value, met:bool}]}.
- Внутрішній seam (не HTTP): розширити `telemetry::log` → `telemetry::log_typed(db, TelemetryEvent, ctx)` де ctx несе user_id/event_id/referrer/amount_minor/is_guest/session_id; існуючий `log` лишається як thin-wrapper (best-effort, infallible для caller — telemetry write НЕ ламає виміряну дію).
- Server-side емісія додається в: webhooks/payments (checkout_paid/failed з amount_minor), tickets.rs `validate` (scan_result{result: ok|already_used|inactive}), tickets transfer/claim (gift_sent/gift_claimed з referrer_user_id), reserve/checkout (checkout_started{amount_minor,is_guest}), RSVP/free-ticket (rsvp), invite-accept (invite_accepted).

**UI:**
- Admin `Analytics` (нова сторінка `admin/app/analytics` під наявним admin-layout): секція North-Star — картки tickets/active-user 28d, active_users, k_factor з pruning-світлофором (зелений/червоний проти порогу).
- Admin Analytics — Purchase-funnel: горизонтальний бар-стек сходинок з count + drop% між сходинками; фільтр діапазону дат і опційно event_id.
- Admin Analytics — Timeseries: лінійний графік active_users/signups/tickets з перемикачем bucket day|week; підписи MAU/D7/D30.
- Admin Analytics — Growth goals: таблиця pruning-цілей [share→claim ≥30%, guest→signup ≥40%, K ≥0.5] зі значенням і met/not_met.
- БЕЗ змін у Android/Web user-facing екранах поза додаванням best-effort emit-викликів (event_view при відкритті деталей події, tier_select при виборі tier, share_clicked/app_open/session_start).

**Acceptance Criteria:**
- AC-1: Існує `docs/analytics/tracking-plan-v1.md` з реєстром рівно канонічних подій (event_view, tier_select, checkout_started, checkout_paid, checkout_failed, rsvp, gift_sent, gift_claimed, invite_sent, invite_accepted, share_clicked, scan_result, onboarding_step, app_open, session_start, signup, ticket), кожна з: name, required props, optional props, тип кожної property, point-of-emit (client|server + модуль).
- AC-2: `TelemetryEvent` enum + schema-map у Rust є єдиним runtime-джерелом валідації; додати подію = один enum-варіант + запис у map (компіляція падає, якщо map неповний).
- AC-3: POST /api/telemetry з невідомим `name` → 400; з відомим name але відсутнім required-property → 400 з переліком відсутніх; зайві props не персистяться.
- AC-4: checkout_started емітиться server-side при reserve/checkout з `amount_minor` (minor-units) та `is_guest`; checkout_paid/checkout_failed — з webhook/payments; жодна з цих подій не втрачається при успішній зміні стану (транзакційно узгоджено або best-effort після commit — визначено й задокументовано).
- AC-5: scan_result емітиться з `validate` для всіх трьох гілок (ok|already_used|inactive) з event_id; attended у воронці = count(scan_result{result:ok}).
- AC-6: gift_claimed та invite_accepted несуть розв'язаний `referrer_user_id` (з share/gift-токена), утворюючи пару з gift_sent/invite_sent того ж source-користувача; пара обчислювана SQL-join-ом.
- AC-7: GET /api/admin/telemetry/funnel повертає сходинки у фіксованому порядку з коректними step_conversion% і drop% (drop% сходинки N = 1 − count[N]/count[N−1]); порожній діапазон → нулі, не 500.
- AC-8: GET /api/admin/telemetry/northstar рахує tickets_per_active_user за 28-денним вікном (paid + free/RSVP у чисельнику, distinct active_users у знаменнику), k_factor = invites_accepted/active_users, і повертає `goals` з met:bool проти порогів 0.30/0.40/0.50.
- AC-9: Усі три admin-endpoint вимагають CurrentAdmin; анонім/звичайний user → 401/403; ендпоінти не повертають PII (лише агрегати/id).
- AC-10: MAU/D7/D30 обчислюються з app_open|session_start подій; timeseries bucket day|week агрегує коректно за `created_at` у event_timezone-agnostic UTC-бакетах.
- AC-11: Наявний GET /api/admin/telemetry і всі поточні emit-виклики (signup/save/follow/ticket) продовжують працювати без регресії (backward-compat).
- AC-12: telemetry write НІКОЛИ не змінює результат/латентність виміряної дії настільки, щоб її зламати: помилка запису логується warn і ковтається (як у поточному `log`).

**Security-вектори (SEC):**
- SEC-1: Admin-only enforcement — funnel/timeseries/northstar доступні ЛИШЕ CurrentAdmin; тест підтверджує 401 (анонім) і 403 (звичайний user); жодного leak агрегатів не-адміну.
- SEC-2: PII-leak guard — валідатор POST /api/telemetry ВІДХИЛЯЄ/дропає props з email/іменами/токенами; тест перевіряє, що навіть якщо клієнт надішле зайвий `email`, він не персиститься; admin-відповіді не містять user email/name.
- SEC-3: Referrer-spoofing — `referrer_user_id` НІКОЛИ не береться з клієнтського payload; тільки резолвиться сервером з валідного, не-простроченого, single-use share/gift-токена; підроблений/чужий referrer у тілі запиту ігнорується (тест: клієнт шле referrer_user_id → відкидається).
- SEC-4: K-factor inflation / self-referral — self-accept (referrer_user_id == user_id) не рахується у k_factor; повторний claim того самого single-use токена не інкрементить invites_accepted (idempotent за токеном); тест на self-referral і replay.
- SEC-5: Telemetry-flood / abuse — POST /api/telemetry за rate-limit (per-IP і per-user); анонімний потік не роздуває counts і не викликає DoS БД; тест на перевищення ліміту → 429.
- SEC-6: Injection у props — name обмежений enum-ом (не довільний TEXT у нових подіях); props валідуються за типами; JSONB-запис параметризований (без SQL-injection); тест з `name` що містить SQL/довгий payload → 400.
- SEC-7: Amount-integrity — `amount_minor` приймається лише як non-negative BIGINT (minor-units); від'ємні/float/overflow значення → 400, щоб не отруїти GMV-агрегати.

**Test Matrix:**
- POS: POST /api/telemetry event_view з валідними props → 204, рядок з promoted event_id.
- POS: checkout_started server-side емісія при reserve з amount_minor+is_guest; читається у funnel.
- POS: scan_result ok/already_used/inactive — усі три гілки емітяться з validate.
- POS: gift_claimed з валідним токеном → referrer_user_id проставлено; пара з gift_sent join-иться.
- POS: GET funnel повертає 5 сходинок у порядку з коректними drop%.
- POS: GET northstar 28d → tickets_per_active_user, k_factor, goals[].met коректні на seed-даних, що перевищують/не досягають порогів.
- POS: timeseries active_users bucket=day агрегує app_open/session_start у денні бакети.
- POS: backward-compat — GET /api/admin/telemetry (7d counts) і signup/save/follow/ticket emit незмінні.
- NEG: POST /api/telemetry невідоме name → 400.
- NEG: відомий name без required property → 400 з переліком відсутніх.
- NEG: amount_minor від'ємний/float/рядок → 400.
- NEG: порожній діапазон дат у funnel → нулі, не 500.
- NEG: timeseries невалідний metric/bucket → 400.
- [SEC] анонім/звичайний user на /funnel|/timeseries|/northstar → 401/403.
- [SEC] клієнт шле referrer_user_id у props → ігнорується; referrer лише з токена.
- [SEC] self-referral (referrer==self) → не рахується в k_factor.
- [SEC] replay single-use share-токена → invite_accepted не інкрементиться двічі.
- [SEC] props з email/PII → не персиститься; admin-відповіді без PII.
- [SEC] rate-limit flood POST /api/telemetry → 429; counts не роздуті.
- [SEC] name з SQL/overlong payload → 400, без побічних ефектів у БД.
- [SEC] telemetry insert падає (симульовано) → виміряна дія (checkout/scan) все одно успішна (warn залоговано).

---

## SDD-QUICKCREATE — Quick-create події (Luma-mode) + KYC just-in-time + post-publish share-екран

**Мета:** Зробити найчастіший кейс (безкоштовна вечірка) публікованим за <3 хв з одним рішенням «Безкоштовно/Платно», прибрати обов'язковий organizer-онбординг та pre-create KYC (KYC піднімається just-in-time лише на «Запросити виплату»), і замкнути supply-side virality loop через post-publish share-екран (URL + Telegram/WhatsApp/копіювати + QR) та «Дублювати подію». Ціль north-star: підняти organizer activation (create→publish→first-ticket), зафіксувати TTFS як метрику, дати supply-side K-factor. Складне (tiers-editor, sales-windows, visibility) ховається за «Розширеними», а не видаляється.

**Поверхні:** API (api/src/events.rs, api/src/payouts.rs або новий organizer payout-request handler, api/src/telemetry.rs, api/src/routes.rs), Android (ui/create/CreateEventScreen + CreateEventViewModel; новий SharePublishScreen; MyEventsScreen duplicate-action; JIT KYC-банер у payout-екрані), Web (admin/ або web/ create-event форма з Розширеними; post-publish share-модал; organizer payout-request кнопка+банер), PM/UX (TTFS-метрика в tracking plan)  ·  **Effort:** M  ·  **Залежності:** INIT-CONTRACTS (minor-units гроші, event_timezone, start/end layout, i18n-каркас) — щоб не робити breaking-міграцій пізніше, Наявний telemetry-seam (api/src/telemetry.rs) для TTFS-подій, Наявний escrow/create_payout seam (api/src/payouts.rs, escrow.rs) provider-neutral — payout-request перевикористовує його під owner-guard, Наявний uploads.rs для cover picker, TASK-TIERS-1 — лише як майбутнє посилання-заглушка під «Розширеними» (НЕ блокує цю задачу)

**У scope:**
- Quick-create дефолт-форма: title + date/time (starts_at) + city + cover(poster_url) + один toggle Безкоштовно/Платно; при Платно з'являються два поля — price + tickets_total. Публікація одним CTA «Опублікувати».
- «Розширені налаштування» (collapsed за замовчуванням) містять: description, venue_address, ends_at, category (дефолт вгадується/Other), age_rating, links/gallery, status=draft toggle. Tiers-editor/sales-windows/min-max/visibility залишаються під «Розширеними» як заглушка-посилання на TASK-TIERS-1 (НЕ реалізуються тут).
- ends_at обов'язковий для cost_type=paid; дефолт starts_at+4год якщо не задано. Для free — опційний.
- Currency фіксується на рівні події = UAH (дефолт з міста); поле сховане в quick-режимі.
- Прибрати окремий organizer-онбординг як передумову: кнопка «Створити подію» доступна будь-якому authed-користувачу одразу (is_organizer лишається косметичним). display_name(name) редагується inline у формі якщо порожній/дефолтний — PATCH /api/me.
- JIT-KYC: прибрати KYC як крок перед створенням; додати POST /api/organizer/payouts/request що вимагає kyc_status=verified AND cleared_balance>0; якщо kyc не verified → 409 kyc_required; клієнт показує KYC-банер саме тут.
- Post-publish share-екран (Android + Web): показує public URL події, кнопки Telegram / WhatsApp / Копіювати посилання / QR-код. Web endpoint GET /api/events/:id/share повертає share_url + прегенеровані deep-share тексти (uk).
- «Дублювати подію»: POST /api/events/:id/duplicate → нова чернетка (status=draft) з тими ж полями окрім starts_at/ends_at (обнуляються) і tickets_left(=tickets_total).
- TTFS-телеметрія: події event_create_started, event_published, event_first_ticket через наявний telemetry-seam; TTFS = event_published.ts − event_create_started.ts.
- Guest-friendly share URL (публічна сторінка події доступна анонім — вже є GET /api/events/:id для published).

**Поза scope:**
- Ticket tiers editor, sales-windows, min/max per order, visibility unlisted/private — окрема задача TASK-TIERS-1 (тут лише посилання-заглушка під «Розширеними»).
- Guest checkout / gift-token seam (K-factor claim-loop) — окрема INIT/TASK; тут лише share івенту, не білета-в-подарунок.
- Реальний PSP / провайдер-міграція — Фаза 4. payout-request проходить через існуючий admin/escrow seam (provider-neutral).
- Зміна escrow state machine, ledger, KYC-review UI адмінки.
- Redesign discovery/feed.

**Модель даних:**
- БЕЗ нових таблиць для ядра. events вже має всі потрібні колонки (title, starts_at, ends_at, city, poster_url, price, currency, tickets_total/left, cost_type, status draft|published, category default 'Other').
- Міграція 00xx_events_defaults.sql (опційно, LOW): нічого структурного — лише за потреби backfill category NULL→'Other'; підтвердити що ends_at nullable (є). Уникати breaking-міграцій.
- КYC/payout: використовує наявні users.kyc_status/payout_* (0025_kyc.sql) та payouts (0018). Додати НІЧОГО в схему для payout-request окрім, за потреби, індексу; сам request — це синхронний виклик існуючого escrow-seam create_payout під owner-guard замість admin-only.
- Телеметрія: використати наявний telemetry-seam (api/src/telemetry.rs). Якщо це append-таблиця events/telemetry_events — додати event-типи 'event_create_started'|'event_published'|'event_first_ticket' як значення поля, без нової таблиці.

**API:**
- POST /api/events (існує) — розширити валідацію: якщо cost_type='paid' і ends_at IS NULL → дефолт starts_at+4h (сервер); якщо paid і price<=0 → 422; category опційна на вході (дефолт 'Other'). organizer_id завжди з токена. 201 Event.
- POST /api/events/:id/duplicate (новий, owner-only) → 201 Event: копія полів джерела, status='draft', starts_at=NULL, ends_at=NULL, tickets_left=tickets_total, poster_url скопійовано. НЕ анонсує (draft).
- GET /api/events/:id/share (новий, публічний для published; owner для draft) → 200 {share_url, qr_payload, telegram_url, whatsapp_url, copy_text} — share_url = базовий public-URL події; deep-share тексти uk-локалізовані з title+city+date.
- POST /api/organizer/payouts/request (новий, authed=organizer) → 200 Vec<Payout> АБО 409 {code:'kyc_required'} якщо kyc_status!='verified'; 409 {code:'nothing_to_clear'} якщо cleared cума=0. Проходить через той самий escrow/create_payout seam що й admin (provider-neutral), owner=self.
- PATCH /api/me (існує/розширити) — inline display_name update з форми якщо name порожнє.
- POST /api/telemetry (існує seam) — приймає {kind, event_id?, ts} для 'event_create_started'|'event_published'|'event_first_ticket'. event_published емітиться сервером при status→published; event_first_ticket — у tickets/orders mint-шляху.

**UI:**
- Create-event (quick default): поля [Назва*] [Дата й час* — starts_at picker] [Місто*] [Обкладинка — image picker→uploads] + toggle [Безкоштовно | Платно]. При Платно: [Ціна, ₴*] [Кількість квитків*]. CTA [Опублікувати]. Валідація кнопки inline. Inline [Ваше ім'я] якщо name порожнє.
- «Розширені налаштування» (collapsible, згорнуто): Опис, Адреса, Час завершення (ends_at picker; required-маркер при Платно), Категорія, Вік (age_rating), Посилання, Зберегти як чернетку (status=draft). Плейсхолдер-рядок «Кілька типів квитків / вікна продажу — скоро» (лінк на TASK-TIERS).
- Post-publish SharePublishScreen: заголовок «Готово! Подія опублікована», великий public URL з кнопкою Копіювати, кнопки [Telegram] [WhatsApp] [QR], прев'ю обкладинки, вторинна дія [Переглянути подію] / [До моїх подій].
- MyEventsScreen: у меню кожної події дія [Дублювати] → відкриває quick-create prefilled draft.
- Organizer payout-екран: кнопка [Запросити виплату] active лише коли cleared>0; при kyc не verified — inline-банер «Підтвердіть особу для виплат» → перехід на KYC-форму (POST /api/me/kyc). БЕЗ pre-create KYC-стіни.

**Acceptance Criteria:**
- AC-1 Authed-користувач БЕЗ будь-якого organizer-онбордингу і БЕЗ KYC відкриває create, заповнює title+date+city+cover, лишає toggle=Безкоштовно, тисне «Опублікувати» → 201, подія published, cost_type='free', видима в /api/events та /api/me/events.
- AC-2 Toggle=Платно з price>0 і tickets_total>0 → 201; якщо ends_at не задано, сервер проставляє starts_at+4h; якщо price<=0 при Платно → 422.
- AC-3 «Розширені» згорнуті за замовчуванням; жодне з полів під ними не обов'язкове для free-публікації (окрім ends_at для paid).
- AC-4 POST /api/events/:id/duplicate (owner) → 201 нова подія status='draft', starts_at/ends_at=NULL, tickets_left=tickets_total, ті самі title/price/city/poster; НЕ анонсована (нема follower fan-out/wall-post).
- AC-5 GET /api/events/:id/share для published повертає непорожні share_url, telegram_url, whatsapp_url, copy_text, qr_payload; клієнт показує share-екран одразу після публікації.
- AC-6 POST /api/organizer/payouts/request при kyc_status!='verified' → 409 {code:'kyc_required'}; при verified+cleared>0 → 200 з payout(-ами) через escrow-seam; при cleared=0 → 409 {code:'nothing_to_clear'}.
- AC-7 TTFS вимірний: event_create_started і event_published емітяться з event_id; для однієї сесії TTFS обчислюється; event_first_ticket емітиться при першому mint для події.
- AC-8 «Створити подію» доступна будь-якому authed без is_organizer=true; display_name редагується inline при порожньому name (PATCH /api/me).
- AC-9 Duplicate/share/payout-request — owner-only: не-власник → 403; неіснуюча подія → 404.

**Security-вектори (SEC):**
- SEC-1 organizer_id/owner береться виключно з токена; duplicate/share(draft)/payout-request перевіряють require_owner (403 чужому, 404 неіснуючому) — анти-spoof.
- SEC-2 POST /api/organizer/payouts/request суворо гейтить kyc_status='verified' AND blocked=false (не можна виплатити заблокованому/неверифікованому); сума й order-selection формуються сервером із escrow 'cleared', не з тіла запиту.
- SEC-3 share_url/qr_payload не містять секретів чи owner-only токенів — це публічний event deep-link; draft-share доступний лише owner.
- SEC-4 Валідація/параметризовані запити на всіх нових endpoint; брудний ввід (від'ємна ціна, порожня назва, ends_at<starts_at) → 422, не 500; ends_at<starts_at відхиляється.
- SEC-5 duplicate не копіює sold/tickets_left джерела як «продані» — обнуляє до tickets_total (щоб не було фантомних проданих/escrow-станів); не копіює status=published (завжди draft), щоб уникнути обходу announce.
- SEC-6 Телеметрія-endpoint приймає лише whitelisted kind-значення, rate-limited, і НЕ довіряє client-ts для безпекових рішень (лише метрика); event_id перевіряється на існування.
- SEC-7 payout-request rate-limit / ідемпотентність, щоб повторний клік не створював дублі payout поверх того самого cleared-набору (той самий інваріант що в admin create_payout).

**Test Matrix:**
- [POS] AC-1 free quick-publish → 201 published/free (integration)
- [POS] AC-2 paid без ends_at → сервер проставляє +4h; paid з ends_at → зберігає (integration)
- [NEG] AC-2 paid з price<=0 → 422; tickets_total<0 → 422 (integration)
- [NEG] ends_at<starts_at → 422 (integration, SEC-4)
- [POS] AC-4 duplicate → draft, дати NULL, tickets_left=tickets_total, no-announce (integration)
- [SEC] AC-9/SEC-1 duplicate/share/payout не-власником → 403; неіснуюча → 404 (integration)
- [POS] AC-5 share для published → всі поля непорожні (integration)
- [SEC] SEC-3 share draft: owner 200, анонім/чужий → 403/404; share_url без секретів (integration)
- [POS] AC-6 payout-request verified+cleared>0 → 200 payout (integration)
- [NEG][SEC] AC-6/SEC-2 payout-request kyc!=verified → 409 kyc_required; blocked user → відмова; cleared=0 → 409 nothing_to_clear (integration)
- [SEC] SEC-7 подвійний payout-request → без дублів payout по тому самому cleared-набору (integration)
- [POS] AC-7 event_create_started+event_published емітяться, TTFS обчислюється; event_first_ticket при першому mint (integration)
- [SEC] SEC-6 telemetry невалідний kind → відхилення; неіснуючий event_id → відхилення; rate-limit (integration)
- [POS] AC-8 authed без is_organizer створює подію; inline display_name PATCH /api/me (integration)
- [MANUAL] Android: TTFS<3хв для free quick-publish на пристрої; share-екран Telegram/WhatsApp/QR/copy працює
- [MANUAL] Web: Розширені згорнуті; post-publish share-модал; payout-request KYC-банер JIT

---

## SDD-CHECKOUT-1 — Простий за замовчуванням checkout + all-in ціна + native-wallet express (provider-agnostic)

**Мета:** Зробити купівлю/отримання квитка максимально простою і конверсійною: один CTA за замовчуванням (1 tier × 1 qty), all-in ціна без окремого рядка "сервісний збір", free/RSVP → миттєвий mint у bottom-sheet без проміжного екрана, native-wallet express-кнопка над "Купити", детермінований post-payment (processing/success/failed + idempotent retry з авто-poll), organizer fee-transparency у create. Усе поверх незмінного provider-agnostic seam (checkout повертає provider_action). Рухає north-star (квитки на активного користувача) через checkout completion rate і прибирає антипатерн-fee як причину №1 відтоку.

**Поверхні:** API, Android, Web  ·  **Effort:** L  ·  **Залежності:** INIT-CONTRACTS / TASK-PROV-1 (trait PaymentProvider + Noop/Test + registry + provider_action), TASK-TIERS-1 (ticket_tiers: tier_id, per-tier sold/capacity/max_per_order) — для повного tier×qty; до злиття fallback на дефолтний Standard tier, TASK-AUTH-1 (Google-only Sign-In) — для frictionless guest→signup claim, Наявний код: api/src/orders.rs (checkout/fulfill/pay_test), api/src/tickets.rs (reserve/mint_ticket/decrement_tickets), api/src/payments.rs (load_settings), web/app/events/[eventId]/EventDetailClient.tsx (розчепити payTest-coupling), android .../tickets/TestPayScreen.kt + TicketsViewModel.kt, Міграція 0033_checkout.sql (orders.provider_ref/fee_amount/quantity/guest_*/claim_token + order_tickets)

**У scope:**
- Прогресивне розкриття checkout: дефолт 1 tier (перший public активний) × qty=1 × один CTA; promo/gift/guest у згорнутих disclosure-секціях
- All-in ціна: order.amount ВЖЕ включає platform_fee; UI показує 'Ціна квитка' та 'ВСЬОГО' одним числом, розбивка fee — під expandable 'Деталі ціни' (read-only)
- Escrow-згода як пасивний disclosure-текст (не required checkbox) для free та простих paid; НЕ блокує CTA
- Free/RSVP шлях: cost_type='free' або обраний tier.price=0 → reserve() mint одразу з bottom-sheet поверх Деталей (без екрана Checkout)
- checkout() повертає provider_action {kind: native_sheet|redirect|test_autopay|none, url?, provider_ref?}; клієнти діють лише за kind — жодного захардкодженого payTest/LS URL
- native_sheet як express-кнопка (Apple/Google Pay) над 'Купити'; provider_action.kind='native_sheet' від Noop/TestProvider у dev/test симулює миттєвий success
- Детермінований post-payment: GET /api/orders/:id/status → {state: pending|processing|paid|failed, ticket_id?, failure_reason?}; клієнт авто-poll 'Готуємо твій білет…' до paid/failed
- Idempotent retry оплати: POST /api/orders/:id/pay-test та generic-webhook fulfill ідемпотентні (дубль → той самий ticket, без double-charge/double-mint)
- quantity у checkout: stepper 1..min(tickets_left/tier.left, max_per_order); mint N квитків в одному order за одну транзакцію
- Organizer create-event fee-transparency: під полем ціни tier — рядок 'Покупець платить X · Ви отримаєте Y' (розрахунок від platform_fee_bps через GET /api/checkout/quote або локально)
- guest checkout (web, free-only на цій фазі): checkout/reserve {guest:{email,name}} → single-use short-TTL signed claim-token; e-ticket на /t/[token]
- Web: прибрати безумовний payTest() з EventDetailClient.confirmBuy; діяти за provider_action
- Android: TestPayScreen → детермінований post-payment екран з авто-poll; bottom-sheet для free

**Поза scope:**
- trait PaymentProvider та registry (передумова INIT-CONTRACTS/TASK-PROV-1 — тут споживається, не будується)
- ticket_tiers модель+міграція (передумова TASK-TIERS-1; тут споживається tier_id у checkout)
- LemonSqueezyProvider / реальний PSP / реальні native-wallet SDK-креденшли (Фаза 4)
- ledger_entries (TASK-LEDGER-1)
- gift-at-checkout з реальним mint на отримувача (окрема TASK-GIFT; тут лише згорнута UI-секція-заглушка, не required)
- paid guest checkout (лише free на цій фазі)
- rotating QR (TASK-QR-1)
- admin escrow refund/release через seam (TASK-ADMIN-SEAM-1)
- escrow sweep/payout логіка

**Модель даних:**
- orders: додати provider_ref TEXT NULL, provider TEXT (є), fee_amount NUMERIC(12,2) NOT NULL DEFAULT 0 (розбивка all-in: amount=ticket_price*qty+fee_amount), quantity INT NOT NULL DEFAULT 1, guest_email TEXT NULL, guest_name TEXT NULL, claim_token TEXT NULL UNIQUE, claim_expires_at TIMESTAMPTZ NULL — міграція 0033_checkout.sql
- orders.escrow_status: розширити коментар станів (none|held|...); статус 'processing' моделюється в orders.status (pending|processing|paid|failed) — додати 'processing' до допустимих значень
- orders.tier_id UUID NULL REFERENCES ticket_tiers(id) (FK м'який до TASK-TIERS; якщо tiers ще нема — колонка nullable, дефолтний tier)
- Індекс: orders_claim_token_idx ON orders(claim_token) WHERE claim_token IS NOT NULL
- Один order → N tickets: додати order_id UUID NULL REFERENCES orders(id) у tickets АБО таблиця order_tickets(order_id, ticket_id) — обрати order_tickets(order_id UUID, ticket_id UUID, PRIMARY KEY(order_id,ticket_id)) щоб не ламати наявний orders.ticket_id (лишити ticket_id як 'первинний' для qty=1 сумісності)
- payment_settings: без змін схеми (provider вже є); native_sheet-capability описується в provider registry, не в БД

**API:**
- POST /api/events/:id/checkout {tier_id?, quantity=1, promo?, gift?{recipient_email,message}, guest?{email,name}} → 201 OrderIntent{order_id, provider, amount_minor|amount, currency, fee_amount, quantity, provider_action{kind:native_sheet|redirect|test_autopay|none, url?, provider_ref?}}
- GET /api/orders/:id/status → 200 {order_id, state:pending|processing|paid|failed, ticket_ids:[], failure_reason?} (auth: buyer або guest-claim-token) — джерело детермінованого post-payment poll
- POST /api/orders/:id/pay-test → ідемпотентний mint (test_autopay); повторний виклик по paid order → 200 з тими самими ticket_ids (не 409/не double-mint)
- POST /api/events/:id/reserve {tier_id?, quantity=1, guest?{email,name}} → 201 {ticket_ids[], claim_token?} — free/RSVP, mint без order-екрана
- GET /api/checkout/quote?event_id=&tier_id=&quantity= → 200 {ticket_price, fee_amount, total, currency, organizer_receives} — живить all-in UI та organizer fee-transparency
- POST /api/webhooks/:provider (generic) → verify_webhook → fulfill (ідемпотентно за provider_ref); kind=ChargeFailed → order.status=failed+failure_reason
- GET /api/t/:claim_token → 200 e-ticket (guest, single-use view; не мутує) — публічний claim перегляд
- POST /api/claim/:claim_token {id_token?} → прив'язати guest-квиток до акаунта після frictionless signup (Google) — single-use, TTL

**UI:**
- Деталі події (Android+Web): вибір tier (авто-обраний перший public) + qty stepper + один CTA 'Купити'/'Отримати'(free); express native-wallet кнопка над CTA якщо provider_action=native_sheet; promo/gift/guest — згорнуті disclosure
- Checkout bottom-sheet (paid): 'Ціна квитка' + 'ВСЬОГО' (all-in, fmtPrice+currency), expandable 'Деталі ціни' (subtotal, сервіс, всього), qty stepper, згорнутий promo, пасивний escrow-disclosure текст, CTA 'Оплатити'
- Free/RSVP bottom-sheet поверх Деталей: миттєвий mint, без проміжного екрана, success-стан → 'Квиток у Мої квитки'
- Post-payment екран/стан (Android TestPayScreen replace + Web): processing 'Готуємо твій білет…' зі спінером+авто-poll; success → e-ticket; failed → причина + кнопка 'Спробувати ще' (idempotent retry)
- Organizer create-event (Web EventForm + Android CreateEvent): під tier.price — рядок 'Покупець платить X · Ви отримаєте Y' (live-перерахунок від platform_fee)
- Web guest checkout: поля email+name у disclosure; після success → /t/[token] з CTA 'Зареєструватися і забрати' (Google one-tap)

**Acceptance Criteria:**
- AC-1 Деталі події за замовчуванням показують один CTA ('Купити' для paid, 'Отримати' для free) з передобраним першим public активним tier і qty=1; promo/gift/guest секції згорнуті і не потрібні для завершення покупки
- AC-2 Ціна показується all-in: 'ВСЬОГО' == order.amount (ticket_price*qty + fee_amount); окремого видимого рядка 'сервісний збір' на первинному екрані немає — лише під expandable 'Деталі ціни'
- AC-3 GET /api/checkout/quote повертає ticket_price, fee_amount, total (=ticket_price*qty+fee_amount), organizer_receives (=ticket_price*qty - platform_fee частка організатора) для заданих event/tier/qty
- AC-4 Free-подія або tier.price=0: reserve() мінтить N квитків одразу з bottom-sheet поверх Деталей, без екрана Checkout; повертає ticket_ids; escrow не залучається
- AC-5 checkout() повертає provider_action.kind ∈ {native_sheet, redirect, test_autopay, none}; клієнт діє лише за kind; у test-режимі TestProvider повертає test_autopay (або native_sheet у симуляції) — жодного захардкодженого LS URL чи безумовного payTest у клієнті
- AC-6 quantity>1: один order з quantity=N мінтить рівно N квитків в одній транзакції; tickets_left/tier.left зменшено на N атомарно; over-sell неможливий
- AC-7 GET /api/orders/:id/status повертає детермінований state (pending→processing→paid|failed); клієнт авто-poll до термінального стану; UI показує 'Готуємо твій білет…' у processing
- AC-8 Idempotent retry: повторний pay-test/webhook-fulfill по вже-paid order повертає ті самі ticket_ids без нового mint і без зміни tickets_left (жодного double-charge/double-mint)
- AC-9 Escrow-згода — пасивний disclosure-текст, не required checkbox для free/простих paid; CTA активний без відмітки
- AC-10 Organizer create-event показує 'Покупець платить X · Ви отримаєте Y' з коректним розрахунком за поточним platform_fee_bps при зміні ціни tier
- AC-11 Guest (web, free): checkout/reserve з guest{email,name} → single-use claim_token з TTL; /t/:token показує e-ticket; POST /api/claim/:token прив'язує до акаунта і робить token недійсним
- AC-12 failed-стан: provider ChargeFailed → order.status=failed + failure_reason; UI показує причину і 'Спробувати ще', що створює новий charge на тому ж order (idempotent, без дубль-квитка)

**Security-вектори (SEC):**
- SEC-1 Idempotency проти double-charge/double-mint: fulfill бере order FOR UPDATE, мінтить лише якщо status='pending'; provider_ref унікальний — повторний webhook/pay-test = no-op, ті самі ticket_ids
- SEC-2 guest claim_token: single-use, short-TTL (≤72h), криптографічно-стійкий (не guessable, не sequential); після claim/expiry — 410/404; rate-limit на /t/:token та /claim/:token (device/email-cap проти масового мінту)
- SEC-3 pay-test лише buyer власником order і лише mode=test; чужий/невідомий order → 404 (не 403, щоб не розкривати існування)
- SEC-4 all-in amount рахується сервером від price+platform_fee_bps; клієнт НЕ передає amount/fee — інакше маніпуляція ціною; quote лише інформативний
- SEC-5 quantity обмежений сервером min(left, max_per_order); негативний/0/понад-ліміт qty → 422; атомарний декремент проти over-sell при паралельних checkout
- SEC-6 age-gate 18+: якщо подія має age_rating і у користувача нема/не підходить DOB → інлайн-підказка (не глухий 409); guest без DOB не може claim 18+ квиток до підтвердження віку
- SEC-7 provider_action.url (redirect) лише з довіреного provider-конфігу (не user-controlled) — інакше open-redirect; native_sheet provider_ref не приймається від клієнта при fulfill (лише від verified webhook/seam)
- SEC-8 escrow-інваріант: free/guest/quote/status endpoints НЕ рухають грошей і не викликають provider.transfer/refund; лише charge/reserve через seam

**Test Matrix:**
- [POS] AC-1: Деталі paid-події → один CTA, tier передобраний, promo/gift згорнуті
- [POS] AC-2/AC-3: quote+UI показують all-in ВСЬОГО = ticket_price*qty+fee; expandable розбивка коректна
- [POS] AC-4: free-подія reserve qty=2 → 2 квитки, без Checkout-екрана, escrow_status='none'
- [POS] AC-5: checkout у test → provider_action.kind=test_autopay; клієнт викликає pay-test за kind
- [POS] AC-6: checkout qty=3 → 3 квитки в одному order, tickets_left -3, order.quantity=3
- [POS] AC-7: order.status послідовність pending→processing→paid видима через /status poll
- [POS] AC-10: organizer змінює price tier → 'Ви отримаєте Y' перераховується
- [POS] AC-11: web guest free-checkout → /t/token e-ticket → claim прив'язує до акаунта
- [NEG] AC-6: qty > min(left,max_per_order) → 422; qty=0/від'ємне → 422
- [NEG] AC-4: reserve на paid-tier (price>0) → 409 (платні через checkout)
- [NEG] AC-12: provider ChargeFailed → order.status=failed + failure_reason; retry створює новий charge, НЕ новий квиток
- [NEG] checkout live-режим без сконфігуреного provider → 409 (не orphan pending)
- [SEC] SEC-1: подвійний webhook/pay-test по paid order → ті самі ticket_ids, tickets_left не змінився, один mint
- [SEC] SEC-2: /t/:token після claim → 410; expired token → 404; brute-force sequential token → 404 + rate-limit
- [SEC] SEC-3: pay-test чужого order → 404; pay-test у live → 409
- [SEC] SEC-4: клієнт підмінює amount/fee у checkout body → сервер ігнорує, рахує сам; total від сервера
- [SEC] SEC-5: 100 паралельних checkout на tier з left=5 → рівно 5 успішних, решта 409, без over-sell
- [SEC] SEC-6: 18+ подія без DOB → інлайн age-gate підказка (не глухий 409)
- [SEC] SEC-7: provider_action.url — з конфігу; спроба fulfill з клієнтським provider_ref ігнорується
- [SEC] SEC-8: quote/status/reserve/guest не викликають provider.transfer/refund (grep-guard + тест не-мутації escrow)

---

## SDD-DISCOVERY-1 — Time-aware персональна Афіша: сектори замість плоского фільтр-екрана

**Мета:** Зробити «що робити сьогодні ввечері» first-class: дефолтний екран Афіші — вертикаль готових секцій без жодного тапу («Сьогодні ввечері поруч» → «Для тебе» → «Друзі йдуть» → «Цими вихідними»), з гарантовано непорожнім «Для тебе» через fallback-ланцюг (граф → інтереси/місто → trending+near) для cold-start/аноніма. Фільтри згорнути до одного рядка швидко-чіпів + bottom-sheet. У картці — time+distance+friends-going сигнали. Мета: підняти activation (card→detail CTR, час до першого open), D1-retention, search-success-rate, не додаючи ваги екрану (простота за замовчуванням, глибина за запитом).

**Поверхні:** API (Rust/Axum) — api/src/discover.rs розширення + маршрут у routes.rs, Android (Kotlin/Compose) — ui/afisha/AfishaScreen.kt + EventCard + фільтр bottom-sheet, UX — сектори, quick-chips, empty/cold-start стани  ·  **Effort:** M  ·  **Залежності:** Наявний discover.rs ранжувальний seam + HAVERSINE (reuse, не переписувати), eventsocial.rs friends_going логіка (адаптувати у batch-агрегацію), telemetry.rs — для discover_* подій (адитивне розширення, не новий екран), cities gazetteer (country/lat/lng; опційно cities.tz — якщо колонки нема, UTC-fallback, без міграції), user_prefs (categories, city, near_me/radius) — наявне, Android EventCard/AfishaScreen + дизайн-токени SDD-014 (relative-time, укр-дати, EmptyState/ErrorState), Block/Mute (user_blocks) — якщо ще не змерджено, SEC-2 деградує до blocked=true фільтра (не блокує задачу)

**У scope:**
- Новий read-only endpoint GET /api/discover/sections — сектори з єдиної ранжувальної SQL (reuse discover.rs seam), кожен сектор із заголовком, reason і подіями (макс 12 на сектор)
- Fallback-ланцюг у секторі 'Для тебе': (1) граф+інтереси → (2) інтереси/місто → (3) trending+near — сектор НІКОЛИ не порожній, поки в БД є ≥1 майбутня published-подія
- Time-window бакети на боці API: today_evening (starts_at у [now, кінець today-вечора в межах міста/UTC], distance-first), weekend (найближчі Sat–Sun), friends_going (події, де є friends_going>0)
- Batch friends-going: розширити DiscoverEvent полями friends_going_count:i64 + friends_going_avatars:Vec<UserBrief> (top 3) без N+1
- Time+distance сигнал у EventCard (Android): relative-time badge ('Сьогодні · 19:00', 'Сб', 'Через 2 год') + '2.3 км' коли distance_km відомий
- Android AfishaScreen: дефолт — LazyColumn секторів (горизонтальні каруселі) БЕЗ тапу; search+сегменти+6 рядів фільтрів прибрати з дефолту
- Рядок швидко-чіпів: Сьогодні · Вихідні · Безкоштовні · Поруч (toggle) + кнопка 'Фільтри' → bottom-sheet (місто/дата-діапазон/категорія/ціна/сортування)
- Cold-start / empty / zero-result стани специфіковані (див. AC-7..AC-9)
- Tracking-події через наявний telemetry seam: discover_section_impression, discover_card_open, discover_chip_toggle, discover_filter_apply
- Курсор-пагінація 'показати ще' у розкритому секторі (?section=&cursor=)

**Поза scope:**
- RSVP going/interested як окрема кнопка/об'єкт (going = active/used ticket, interested = event_saves — існуюча модель; НЕ вводимо новий RSVP endpoint у цій задачі)
- Web SSR/SEO-лендінги /c/[category], /city/[slug], JSON-LD (окрема TASK-DISCOVER-WEB)
- Ticket tiers, checkout, gifting, guest-claim — інші ініціативи
- Зміни схеми грошей/escrow/payment seam (задача read-only по discovery)
- Персональні ML-моделі/embeddings — тримаємо просте SQL-ранжування, що вже є
- event_timezone міграція (використовуємо наявні starts_at/ends_at UTC; вечірнє вікно рахуємо від міста-viewer або UTC-fallback без нової колонки)
- Push-нагадування (окрема TASK-NOTIF)
- Нова колонка/таблиця — задача НЕ вимагає міграцій БД

**Модель даних:**
- БЕЗ нових таблиць і БЕЗ міграцій: reuse events, event_saves, follows, tickets, user_prefs, cities, reviews
- Читання friends_going: tickets t JOIN follows f ON f.followee_id=t.owner_id AND f.follower_id=$viewer JOIN users u ON u.id=t.owner_id AND u.blocked=false WHERE t.status IN ('active','used') — агрегувати через LATERAL/віконну підзапит-агрегацію по event_id (без N+1)
- Ранжувальний вираз і HAVERSINE — існуючі константи discover.rs (не дублювати; винести спільний SELECT у private-хелпер ranked_events(viewer_id, window, limit, cursor))
- Вечірнє вікно: evening_end = date_trunc('day', now() AT TIME ZONE COALESCE(vc.tz,'UTC')) + interval '1 day' - канонічно: today_evening = starts_at BETWEEN now() AND (кінець доби viewer-city або UTC); tz береться з cities.tz якщо колонка існує, інакше UTC-fallback (перевірити наявність cities.tz; якщо нема — UTC, без міграції)
- Weekend-вікно: найближчі субота 00:00 — неділя 23:59 у tz viewer-city/UTC

**API:**
- GET /api/discover/sections — auth optional. Query: (none обов'язкових). Resp 200: {sections: [{key: 'today_evening'|'for_you'|'friends_going'|'weekend', title_uk, reason, events: [DiscoverEventCard], has_more: bool}]}. Порожні сектори (крім for_you) відсутні у відповіді, а не []
- DiscoverEventCard (розширений DiscoverEvent): наявні поля events + saves_count + saved_by_me + reason + distance_km + НОВЕ friends_going_count:i64 + friends_going_avatars:[{id,name,avatar_url}] (макс 3)
- GET /api/discover/sections?section=<key>&cursor=<opaque> — пагінація одного сектора: {events:[DiscoverEventCard], next_cursor: string|null}. Cursor = base64(score,starts_at,id) стабільний
- GET /api/discover (наявний) — ЗБЕРЕГТИ без breaking-змін (використовується у 'Для тебе'-tab фолбеком); лише додати нові поля friends_going_* до DiscoverEvent (адитивно, не breaking)
- GET /api/events (наявний параметризований пошук) — ЗБЕРЕГТИ; викликається при активації швидко-чіпа/bottom-sheet фільтра, дефолт-екран його НЕ викликає
- POST/DELETE /api/events/:id/save (наявні) — save-toggle зірочки з картки, без змін

**UI:**
- AfishaScreen (default): [wordmark header] → [quick-chips row: Сьогодні·Вихідні·Безкоштовні·Поруч + 'Фільтри'] → LazyColumn секторів. Кожен сектор: заголовок (title_uk) + опц. 'Все >' → горизонтальна карусель EventCard. Порядок: today_evening, for_you, friends_going, weekend
- EventCard: постер, title, relative-time badge ('Сьогодні · 19:00' / 'Сб · 21:00' / 'Через 2 год'), місто + distance ('Київ · 2.3 км'), price/free-мітка, зірочка save, аватар-стек friends_going + 'N друзів іде', reason-мітка (тонка, напр. 'Бо тобі подобається techno')
- Quick-chip toggle → перемикає екран у 'фільтрований' режим: замість секторів — плоский grid із GET /api/events?today=/free=/near= (single-decision, глибина за запитом)
- Filters bottom-sheet: місто, дата (Сьогодні/Завтра/Вихідні/Діапазон), категорія, ціна (max/free), сортування (Дата/Популярність/Поблизу); 'Застосувати' → GET /api/events; 'Скинути' → назад до секторів
- Empty prefs (новачок без інтересів): дружній банер 'Обери інтереси, щоб краще підбирати' → перехід у prefs; сектор 'Для тебе' все одно заповнений trending+near
- Zero-result (фільтр без збігів): EmptyState іконка+'Нічого не знайшли' + CTA 'Скинути фільтри'
- Стани: loading (skeleton-картки в секторах) / error (ErrorState + Повторити) / content — на кожен сектор і на екран

**Acceptance Criteria:**
- AC-1: GET /api/discover/sections без auth повертає ≥1 сектор; сектор for_you присутній ЗАВЖДИ і має ≥1 подію, поки існує хоча б одна майбутня published-подія (fallback trending+near)
- AC-2: Для авторизованого з follow-графом/інтересами сектор for_you ранжований персонально (reason ∈ followed|interest|city|near); при відсутності сигналів — деградує до trending без помилки
- AC-3: today_evening містить лише події зі starts_at у вікні [now, кінець сьогоднішнього вечора у tz viewer-city або UTC-fallback], відсортовані distance-first потім за starts_at; порожній сектор НЕ повертається у відповіді
- AC-4: weekend містить події найближчих Sat–Sun; friends_going містить лише події з friends_going_count>0 і лише для авторизованого (для аноніма сектор відсутній)
- AC-5: Кожен DiscoverEventCard несе distance_km (коли обидві координати відомі) і friends_going_count + до 3 friends_going_avatars; агрегація без N+1 (один запит на сектор)
- AC-6: Android дефолт Афіші рендерить вертикаль секторів БЕЗ жодного тапу; search-bar і рядки фільтрів НЕ показані у дефолті (лише quick-chips + 'Фільтри')
- AC-7 (cold-start): анонім/новачок бачить непорожні today_evening (за гео/IP-міста або дефолт-міста) + for_you(trending) + weekend; friends_going відсутній — не показуємо порожній блок
- AC-8 (empty prefs): новачок без categories бачить банер-CTA на вибір інтересів, але сектори НЕ порожні
- AC-9 (zero-result): застосований фільтр без збігів показує EmptyState + 'Скинути фільтри', що повертає до секторного дефолту
- AC-10: Тап по картці відкриває Деталі; емітується discover_card_open з {section, event_id, position, reason}. Скрол сектора у viewport емітує discover_section_impression один раз на сесію-сектор
- AC-11: Наявний GET /api/discover не зазнає breaking-змін (нові поля адитивні); наявний GET /api/events семантика фільтрів незмінна
- AC-12: Пагінація GET /api/discover/sections?section=&cursor= стабільна (без дублів/пропусків між сторінками) і повертає next_cursor=null у кінці
- AC-13: relative-time badge локалізований укр ('Сьогодні', 'Завтра', 'Сб', 'Через N год'), дати у форматі '9 чер · 19:00' (звірка з дизайн-токенами SDD-014)

**Security-вектори (SEC):**
- SEC-1: GET /api/discover/sections без auth НЕ витікає приватних сигналів — friends_going і персональні reason доступні лише авторизованому; анонім отримує тільки публічні trending/near/weekend
- SEC-2: friends_going рахує лише взаємно видимих користувачів — виключати blocked=true і (коли існує user_blocks) заблокованих обома напрямками; не розкривати відвідування приватних осіб поза графом viewer
- SEC-3: Жодного SQL string-building з user input — cursor парситься у типізовані (Decimal score, DateTime, Uuid) з валідацією; window/section — closed enum, невідоме значення → 400, не інтерполюється в SQL
- SEC-4: distance_km не розкриває точну адресу приватної особи — лише event↔city great-circle (як зараз у discover.rs), не координати користувачів
- SEC-5: Rate-limit на /api/discover/sections (анонім за IP) — запобігти скрейпінгу каталогу; ліміт узгодити з наявним middleware
- SEC-6: Лише status='published' і (starts_at IS NULL OR >= now()); unlisted/private/draft НЕ протікають у жоден сектор; виключати власні події viewer (як у наявному discover)
- SEC-7: Telemetry-події не логують PII (лише user_id з auth-context, event_id, section, position) — узгоджено з telemetry.rs seam
- SEC-8: Пагінаційний cursor непідробний для ескалації — не містить прихованих фільтрів обходу published-гейта; гейт застосовується завжди на сервері незалежно від cursor

**Test Matrix:**
- [POS] анонім → GET /sections: for_you непорожній (trending+near), today_evening за дефолт-містом, friends_going відсутній
- [POS] авторизований з follow+інтересами → for_you персональний, reason коректний, порядок за score DESC
- [POS] today_evening: подія starts_at через 2 год у місті viewer потрапляє; подія завтра — ні; distance-first сортування
- [POS] weekend: подія в найближчу суботу потрапляє, наступного тижня — ні (межа tz)
- [POS] friends_going_count+avatars: 5 друзів going → count=5, avatars=3; без друзів → сектор відсутній
- [POS] пагінація section=for_you&cursor: сторінка 2 без дублів зі сторінкою 1, кінець → next_cursor=null
- [NEG] невалідний section=foo → 400; невалідний cursor → 400, не 500
- [NEG] БД без майбутніх подій → sections=[{for_you, events:[]}] з коректним empty-контрактом (не 500)
- [NEG] новачок без user_prefs → for_you фолбек trending, без NULL-паніки в SQL
- [NEG] подія без starts_at → не потрапляє в today_evening/weekend, але може бути у for_you
- [SEC] анонім не отримує friends_going/персональний reason (SEC-1)
- [SEC] заблокований користувач (blocked=true / user_blocks) виключений з friends_going (SEC-2)
- [SEC] draft/unlisted/private/минула подія не протікає в жоден сектор (SEC-6)
- [SEC] cursor із підробленим фільтром не обходить published-гейт (SEC-8)
- [SEC] SQL-ін'єкція через section/cursor неможлива — параметризовано/enum (SEC-3)
- [SEC] rate-limit спрацьовує при флуді анонімних запитів (SEC-5)
- [UI] дефолт рендерить сектори без тапу; search/фільтр-ряди відсутні (AC-6)
- [UI] quick-chip 'Безкоштовні' → перехід у grid GET /api/events?free=true; 'Скинути' → назад до секторів
- [UI] zero-result фільтра → EmptyState + Скинути (AC-9)
- [UI] relative-time badge коректний укр для сьогодні/завтра/вихідних/годин (AC-13)

---

## SDD-018-TRUST-ESCROW — Escrow-as-trust видимий + типізований dispute/refund workflow + early-payout клапан

**Мета:** Перетворити невидиму escrow-механіку на ПРОДАЮЧИЙ trust-сигнал для покупця/гостя і на керований supply-side клапан для організатора: (1) показати «Гроші захищені до події» + trust-бейджі організатора на всіх точках рішення (EventCard, Деталі, Checkout, gift-landing); (2) замінити заглушку dispute на типізований workflow зі статусом, SLA-таймером, авто-нотифікаціями та прозорим refund-строком/сумою (з UA air-alert авто-резолвером); (3) додати опційний early/instant-payout апсел (+3%) для організатора; (4) трекати payout-latency як supply-side контр-NSM з SLA-алертом. Кожна поведінка провайдер-агностична: гроші рухаються ЛИШЕ через seam `escrow.rs`/`PaymentProvider`, ця задача НЕ реалізує реального PSP — працює під Noop/Test.

**Поверхні:** API (Rust/Axum): dispute endpoints, refund-status, organizer early-payout request, trust-stats, payout-latency metric, air-alert resolver, Android (Compose): escrow-shield badge, dispute form, refund status, organizer trust badges, early-payout upsell, Web (Next.js): escrow-shield on EventCard/Details/Checkout, gift-landing badge, dispute+refund status pages, organizer trust badges, Admin (Next.js): dispute drawer with typed reason/evidence/SLA timer, refund status, payout-latency SLA alert strip  ·  **Effort:** M  ·  **Залежності:** INIT-CHANNELS (нотифікації/deep-link seam для dispute/refund/early-payout alerts — заявлена залежність ініціативи), TASK-PROV-1 (trait PaymentProvider + Noop/Test) — бажано, але не блокує: dispute/refund/release проходять через існуючий admin escrow seam; якщо PROV-1 ще не злитий, early-payout settle робиться через наявний payout DB-flip з TODO-міткою на seam-виклик, Існуючий telemetry seam (api/src/telemetry.rs) — для tracking-подій, Існуючий uploads.rs — для evidence media (SEC-3), Існуючий admin escrow control (admin.rs refund/freeze/release) — resolve повторно використовує його, НЕ дублює грошову логіку, Локалізаційний каркас uk/en (notify_actor pattern) — для нотифікацій і бейджів

**У scope:**
- Trust-бейдж 'Гроші захищені до події' (escrow-shield) на EventCard, Event Details, Checkout summary, gift-landing (guest), e-ticket
- Organizer trust-бейджі: verified, N проведених подій, member since, спорів <X% — обчислені серверно, показані на Деталях події та публічному профілі організатора
- Типізований dispute: enum reason (event_cancelled | event_not_held | ticket_not_received | fake_or_invalid | other), опційний текст + опційні докази (media_urls), видимий статус open→review→resolved, SLA deadline_at, авто-нотифікації покупцю+організатору+admin
- Refund transparency: серверно обчислені refund_amount_minor + refund_policy_reason (held→повне; clearing→за політикою) + видимий статус requested→processing→refunded; повернення проходить через існуючий admin refund seam
- UA air-alert авто-резолвер disput'ів для подій, скасованих через тривогу: детермінований admin/system-тригер, який резолвить dispute у refund з reason=event_cancelled (SEC: тільки system/admin, ідемпотентно)
- Early/instant-payout апсел (+3% fee) на escrow_status in (clearing, cleared): organizer-request endpoint + 'Гроші надійдуть ~DD.MM' estimate, через payout seam
- Payout-latency метрика (ends_at → paid_out) як supply-side контр-NSM + SLA breach admin-алерт
- Telemetry-події для всіх trust/dispute/early-payout взаємодій (розширення існуючого telemetry seam)
- Міграція: orders.dispute_reason, orders.dispute_status, orders.disputed_at, orders.dispute_deadline_at, orders.refund_amount_minor, dispute_evidence (media_urls), payouts.kind (standard|early|instant) + payouts.fee_bps + payouts.est_arrival_at
- Android: escrow-shield composable, dispute-form екран, refund-status екран, early-payout апсел у organizer payouts; Web: ті самі поверхні + gift-landing бейдж; Admin: dispute drawer з SLA-таймером + refund-статус

**Поза scope:**
- Реальний PSP-провайдер (Stripe/LiqPay/LemonSqueezy live) — Фаза 4, окрема задача TASK-PROV-LS-1
- Double-entry ledger_entries — окрема задача TASK-LEDGER-1 (ця задача лише читає/пише escrow_status, ledger не потрібен для видимості)
- Ticket tiers / quantity / guest-checkout платіжний шлях — окремі задачі (gift-landing бейдж працює поверх наявного free/transfer-flow)
- Автоматичний ML risk-scoring / anti-fraud — окрема TASK-RISK-1 (тут лише детермінований спорів-% для бейджа)
- Chargeback-інтеграція з реальним банком/PSP (KPI chargeback-rate трекається лише коли є live PSP)
- Реальний зовнішній air-alert feed integration — авто-резолвер приймає manual/admin-тригер; live alert-API — окремий backlog
- Зміна escrow state-machine станів held→clearing→cleared→paid_out (лишається незмінною; додаємо лише dispute-підстани та early-payout варіант payout)

**Модель даних:**
- migration 0033_dispute_workflow.sql — ALTER TABLE orders ADD COLUMN dispute_reason TEXT (event_cancelled|event_not_held|ticket_not_received|fake_or_invalid|other), dispute_status TEXT NULL (open|review|resolved), disputed_at TIMESTAMPTZ NULL, dispute_deadline_at TIMESTAMPTZ NULL (SLA), dispute_note TEXT NULL, dispute_resolution TEXT NULL (refunded|released|rejected)
- migration 0033 — CREATE TABLE dispute_evidence (id UUID PK, order_id UUID FK orders ON DELETE CASCADE, media_url TEXT NOT NULL, created_at TIMESTAMPTZ DEFAULT now()); INDEX (order_id)
- migration 0034_refund_transparency.sql — ALTER TABLE orders ADD COLUMN refund_amount_minor BIGINT NULL, refund_status TEXT NULL (requested|processing|refunded), refunded_at TIMESTAMPTZ NULL; back-compat: existing amount Decimal лишається, refund_amount_minor заповнюється при dispute/refund
- migration 0035_early_payout.sql — ALTER TABLE payouts ADD COLUMN kind TEXT NOT NULL DEFAULT 'standard' (standard|early|instant), fee_bps INT NOT NULL DEFAULT 0 (override platform_fee_bps з +300 для early), est_arrival_at TIMESTAMPTZ NULL, settled_at TIMESTAMPTZ NULL
- migration 0035 — ALTER TABLE payouts ADD COLUMN payout_latency_seconds BIGINT NULL (computed ends_at→settled_at при settle, для supply-side NSM)
- payment_settings: ADD COLUMN early_payout_fee_bps INT NOT NULL DEFAULT 300, dispute_sla_hours INT NOT NULL DEFAULT 72, payout_sla_hours INT NOT NULL DEFAULT 120 (provider-neutral config)
- READ-ONLY derived (no table): organizer trust-stats = COUNT(events past), MIN(users.created_at)=member_since, verified flag (users.verified), dispute_rate = disputed_orders/total_orders — обчислюється в запиті, кешується опційно

**API:**
- POST /api/orders/:id/dispute {reason: enum, note?: string, evidence_urls?: string[]} → 201 {dispute_status:'open', dispute_deadline_at} — replaces stub; buyer-only; guard escrow_status IN (held,clearing); freezes escrow→frozen; sets disputed_at, dispute_deadline_at=now()+dispute_sla_hours; notifies organizer+admin; idempotent (already-disputed→409)
- GET /api/orders/:id/dispute → {reason, status, deadline_at, note, evidence_urls[], refund_status, refund_amount_minor, resolution} — buyer or organizer or staff-with-refund-perm or admin
- GET /api/orders/:id/refund-status → {refund_status, refund_amount_minor, currency, policy_reason, refunded_at, est_days} — buyer-visible refund timeline
- POST /api/admin/orders/:id/dispute/resolve {resolution: refunded|released|rejected, refund_amount_minor?} → 204 — admin-only; on refunded routes through existing admin escrow refund seam (escrow_status→refunded, ticket returned, seat freed, promo restored); sets dispute_status=resolved, refund_status; notifies buyer+organizer; audited
- POST /api/admin/air-alert/resolve {event_id} → {resolved_count} — system/admin-only; bulk-resolves all open disputes for a cancelled event as refunded/event_cancelled; idempotent; audited
- GET /api/events/:id/trust → {organizer:{verified, events_held_count, member_since, dispute_rate_pct}, escrow:{protected_until_event:true, clearing_window_days}} — public (value-before-auth)
- POST /api/organizer/payouts/request {kind: early|instant} → {payout_id, amount_minor, fee_bps, est_arrival_at} — organizer-only; guard kyc verified + escrow_status IN (clearing,cleared) with no frozen orders; applies early_payout_fee_bps; routes through payout seam
- GET /api/admin/payouts/latency → {p50_seconds, p95_seconds, sla_breach_count, breaches:[{payout_id, organizer, latency_seconds}]} — admin supply-side NSM + SLA alert
- telemetry: POST existing /api/telemetry events — escrow_badge_view, dispute_open, dispute_resolve, refund_view, early_payout_upsell_view, early_payout_request

**UI:**
- EventCard (Android/Web): compact escrow-shield chip 'Гроші захищені до події' (icon + tooltip); no layout break
- Event Details (Android/Web): escrow-shield banner + one-liner explainer + organizer trust row (verified badge, 'N подій проведено', 'на платформі з YYYY', 'спорів <X%')
- Checkout summary (Android/Web): escrow-shield line above ВСЬОГО + consent-checkbox linking escrow/refund policy + 'Гроші утримуються до DD.MM (кінець події)'
- Gift-landing (Web, guest, no-account): escrow-shield badge + organizer trust one-liner ДО auth-wall — frictionless value-before-signup
- Dispute form (Android/Web): typed reason radio (5 options localized), optional note (≤1000), optional evidence upload (0..4 media), submit → shows open status + SLA countdown 'Розглянемо до DD.MM HH:MM'
- Dispute status (Android/Web): stepper open→review→resolved with SLA timer; on resolve shows refund block
- Refund status (Android/Web): requested→processing→refunded timeline + 'Повернемо N ₴ протягом ~D днів' + policy_reason
- Organizer payouts (Android/Web): early-payout upsell card '+3% отримати зараз' with computed net amount + 'Гроші надійдуть ~DD.MM' vs standard estimate
- Admin dispute drawer: typed reason, evidence thumbnails, buyer/organizer/event context, SLA timer (red on breach), resolve actions (refund/release/reject), refund-status, air-alert bulk-resolve button
- Admin payout-latency alert strip: p50/p95 ends_at→paid_out + SLA-breach count deep-linking breached payouts

**Acceptance Criteria:**
- AC-1: escrow-shield 'Гроші захищені до події' відображається на EventCard, Event Details, Checkout summary, gift-landing і e-ticket; текст локалізований (uk/en); тултип пояснює утримання до кінця події
- AC-2: GET /api/events/:id/trust повертає organizer.verified, events_held_count (COUNT минулих подій), member_since (users.created_at), dispute_rate_pct (disputed/total, 0 при total=0), escrow.clearing_window_days — доступний без авторизації
- AC-3: POST /api/orders/:id/dispute приймає typed reason з фіксованого enum; невалідний reason → 422; note>1000 або >4 evidence → 422; успіх створює dispute_status='open', disputed_at, dispute_deadline_at=now()+dispute_sla_hours, escrow_status='frozen'
- AC-4: dispute можливий лише покупцем і лише коли escrow_status IN (held,clearing); повторний dispute на вже спірному → 409; замовлення не своє/не існує → 404
- AC-5: відкриття dispute шле нотифікації організатору і admin (kind='dispute_open') з deep-link на order; локалізовано
- AC-6: GET /api/orders/:id/dispute показує status (open→review→resolved), deadline_at, evidence_urls, refund_status; доступний buyer/organizer/staff(refund)/admin, іншим → 403/404
- AC-7: POST /api/admin/orders/:id/dispute/resolve з resolution=refunded проводить повернення через ІСНУЮЧИЙ admin escrow refund seam (escrow_status→refunded, ticket→returned, seat freed, promo restored), встановлює dispute_status='resolved', refund_status='processing'→'refunded', refund_amount_minor; audited
- AC-8: resolution=released знімає frozen (потребує наявної логіки force для frozen); resolution=rejected лишає кошти, dispute_status='resolved', refund_status=null; всі три шлють нотифікацію покупцю
- AC-9: refund_amount_minor обчислюється серверно за політикою: escrow_status='held' на момент dispute → повна сума; 'clearing' → за clearing-політикою (за замовчуванням теж повна для MVP, поле policy_reason пояснює); показується покупцю ДО підтвердження і в refund-status
- AC-10: POST /api/admin/air-alert/resolve {event_id} масово резолвить усі open-диспути події як refunded з reason=event_cancelled; ідемпотентно (повторний виклик → resolved_count=0); тільки admin/system; audited
- AC-11: POST /api/organizer/payouts/request {kind:early|instant} доступний лише при kyc_status='verified' AND наявності cleared/clearing без frozen; застосовує early_payout_fee_bps (default 300 = +3%); повертає amount_minor нетто, est_arrival_at; проходить через payout seam (Noop/Test settle детермінований)
- AC-12: early-payout НЕ дозволений якщо є frozen/disputed order організатора → 409 з поясненням
- AC-13: payout_latency_seconds записується при settle (ends_at→settled_at); GET /api/admin/payouts/latency повертає p50/p95 і список breaches де latency > payout_sla_hours
- AC-14: усі trust/dispute/early-payout взаємодії емітять telemetry-події через існуючий seam; жодна нова surface не виходить без tracking (принцип 'все вимірюване')
- AC-15: жоден endpoint цієї задачі не викликає провайдер-специфічної логіки; рух грошей — лише через escrow.rs/admin refund seam/payout seam; під активним Noop/Test усе проходить у CI без зовнішніх викликів

**Security-вектори (SEC):**
- SEC-1: dispute — тільки buyer замовлення (buyer_id=user.id); чужий/неіснуючий order повертає 404 (не 403, щоб не розкривати існування); перевірено інтеграційним тестом
- SEC-2: dispute/resolve — тільки CurrentAdmin; organizer або buyer виклик → 403; air-alert resolver — тільки admin/system, не organizer (інакше self-refund abuse)
- SEC-3: evidence upload обмежено 0..4 media_urls, тільки через існуючий uploads.rs (валідований content-type/size); заборонено довільні URL (SSRF/XSS) — приймати лише internal upload-refs
- SEC-4: refund_amount_minor обчислюється ВИКЛЮЧНО серверно за escrow_status на момент dispute; клієнт не може передати суму повернення (інакше over-refund)
- SEC-5: early-payout KYC-gate незмінний (kyc_status='verified' AND not blocked); frozen/disputed orders виключені з розрахунку суми (інакше виплата спірних коштів)
- SEC-6: idempotency — повторний dispute (409), повторний resolve (no-op на вже resolved), air-alert bulk (0 на повторі); FOR UPDATE на order-row при resolve щоб уникнути гонки refund/release
- SEC-7: dispute_status переходи одностороні (open→review→resolved); неможливо reopen resolved через API (запобігає повторному refund тієї ж суми)
- SEC-8: trust-stats endpoint публічний, але НЕ розкриває PII організатора (лише агрегати: verified, counts, member_since year, dispute_rate_pct) — жодних email/order-id
- SEC-9: air-alert resolver і early-payout — усі грошові дії audited (audit::log у тій самій транзакції, §9.1)
- SEC-10: rate-limit на POST dispute per-user (запобігти dispute-spam/DoS на organizer notifications)

**Test Matrix:**
- [POS] dispute з валідним reason=event_not_held на held-order → 201, escrow→frozen, deadline_at=now+72h, нотифікації надіслані
- [POS] GET /api/events/:id/trust для організатора з 3 минулими подіями, 0 disput → events_held_count=3, dispute_rate_pct=0
- [POS] admin resolve refunded → order refunded, ticket returned, seat freed, promo restored, refund_status→refunded, buyer notified
- [POS] air-alert resolve для скасованої події з 2 open-disput → resolved_count=2, обидва refunded/event_cancelled
- [POS] organizer early-payout request (kyc verified, cleared>0, no frozen) → net amount = cleared*(1-(fee_bps+300)/10000), est_arrival_at set, payout kind='early'
- [POS] refund-status на held-dispute → refund_amount_minor=full amount, policy_reason='held_full'
- [POS] payout-latency повертає p50/p95 і breach для payout з latency > payout_sla_hours
- [NEG] dispute з reason='garbage' → 422
- [NEG] dispute з 5 evidence_urls → 422
- [NEG] повторний dispute на frozen-order → 409
- [NEG] dispute на cleared/paid_out order → 409 (кошти вже недоступні)
- [NEG] admin resolve на вже resolved dispute → no-op/409, refund не повторюється
- [NEG] early-payout при kyc_status='pending' → 409
- [NEG] early-payout коли є frozen order → 409
- [SEC] dispute чужого order (інший buyer_id) → 404, не мутує стан
- [SEC] organizer викликає POST /dispute/resolve → 403
- [SEC] organizer викликає air-alert/resolve → 403
- [SEC] клієнт передає refund_amount_minor=999999 у dispute body → ігнорується, сервер обчислює свою суму
- [SEC] evidence_url з зовнішнім http://evil → відхилено (тільки internal upload-ref)
- [SEC] паралельні resolve refunded + release на тому ж order → FOR UPDATE серіалізує, лише один ефект, без подвійного refund
- [SEC] dispute-spam 20 req/min від одного user → rate-limited (429)
- [SEC] trust endpoint не повертає жодного email/PII у JSON
- [POS] під Noop/Test provider повний цикл dispute→resolve→refund і early-payout проходить у CI без зовнішніх викликів

---

## SDD-018 — Ticket-lifecycle як єдиний домен: іменні квитки + системний transfer/gift + per-ticket escrow/refund

**Мета:** Зробити квиток атомарною одиницею життєвого циклу з per-ticket станом і per-ticket часткою escrow, щоб три головні продуктові петлі — дарування (frictionless, включно з guest), повернення коштів за прозорою політикою і обмін на інший tier — працювали коректно, віральність не перетворювалась на скальпінг-канал, а гроші лишались правдивими при частковому refund у multi-ticket ордері. Ціль north-star: перетворити share/gift-лінк у claim→signup→re-share (K-factor ≥ 0.5) без діри в escrow-власності та без double-spend вікна transfer↔scan.

**Поверхні:** API (Rust/Axum: tickets.rs, orders.rs, escrow.rs seam, нова refund_policy.rs), Android (Compose: MyTicketsScreen, ETicketScreen, GiftTicketDialog, RefundConfirmSheet, ExchangeSheet), Web (Next.js: /me/tickets/[id], guest /t/[token] claim, RefundDialog, GiftDialog), Admin (Next.js: order detail drawer — per-ticket escrow breakdown, partial-refund view)  ·  **Effort:** L  ·  **Залежності:** INIT-TICKET-UNCLAIMED — надає guest claim-token seam (генерація/зберігання/single-use/TTL/anti-abuse); тут споживаємо, не дублюємо., INIT-CONTRACTS — незворотні контракти: minor-units гроші, event_timezone (для refund-вікон hours_before_start у правильному TZ), start/end layout; фіксуємо ПЕРЕД цим модулем., TASK-TIERS-1 (ticket_tiers) — per-tier sold/capacity; за відсутності працюємо з дефолтним 'Standard' tier (legacy events.price/tickets_total)., PaymentProvider seam (PROV-1, Noop/Test) — refund/hold проходять через нього; тут викликаємо trait, не конкретний провайдер., TASK-LEDGER-1 (ledger_entries) — необов'язкова для готовності, але per-ticket escrow-переходи мають бути ledger-сумісні.

**У scope:**
- Per-ticket стан-машина: active → {used|returned|transferred|refunded|frozen}; новий квиток отримувача = active; owner-history через таблицю ticket_transfers.
- Системний transfer-seam POST /api/tickets/:id/transfer: атомарно old→transferred + mint new→active на отримувача АБО на pending-claim (guest без акаунта), запис owner-history, decrement/збереження tier.sold без подвійного лічення.
- Ліміт N transfer на квиток (payment_settings.max_transfers_per_ticket, default 3) + cooldown (transfer_cooldown_minutes, default 60) — анти-скальпінг клапан.
- Guest gift: transfer на email/handle без акаунта створює single-use short-TTL signed claim-token; claim до входу мінтить квиток після signup/login (залежність INIT-TICKET-UNCLAIMED дає token+claim seam — тут ми його ЛИШЕ викликаємо, не дублюємо).
- Per-ticket escrow: кожен квиток несе свою частку amount/promo/tier у новій таблиці order_items (одна на квиток), FOR UPDATE при мутації.
- Partial refund 1-з-N: return/refund одного квитка з multi-ticket ордера перераховує tier.sold-- і суму, лишаючи решту квитків active; escrow ордера частково refunded через order_items, ордер лишається held поки є неповернені квитки.
- Refund-політика як конфігурований об'єкт refund_policy (JSONB на event або дефолт на payment_settings): windows[] з {hours_before_start, percent, fee_returned}; HELD-до-старту = 100% включно зі збором.
- Власність refund/dispute переходить до ПОТОЧНОГО owner квитка (для guest — після claim), а не лишається у первинного buyer.
- Атомарність transfer↔scan проти double-spend: обидва беруть FOR UPDATE на рядок квитка; validate повертає вердикт 'transferred' якщо квиток передано.
- 'Обмін' = задокументований transfer-на-інший-tier: return поточного + re-buy різниці (доплата) або re-buy з поверненням — БЕЗ окремого важкого флоу; один helper поверх наявних transfer/return/checkout.
- Оновлення my_tickets/e-ticket DTO: owner_name, tier_name, transfer_count, is_gift, escrow-частка на квиток; сегменти Майбутні|Минулі|Повернені/Подаровані.
- Telemetry-події для virality/refund correctness (ticket_transferred, gift_claimed, ticket_refunded з суми).

**Поза scope:**
- Реальний PSP: усе поверх NoopProvider/TestProvider через payment seam; charge/hold/transfer_payout/refund/verify_webhook — це PROV-задача, не тут.
- Ticket_tiers таблиця сама по собі — приходить з TASK-TIERS-1; тут ми споживаємо tier_id/tier.sold, і якщо tiers ще нема, працюємо з дефолтним 'Standard' tier (legacy events.price).
- event_staff / door-staff делегування + event-scoped validate perms — окрема задача (TASK-STAFF-1); тут validate лише додає вердикт 'transferred' і FOR UPDATE-атомарність.
- Rotating/short-lived QR (HOTP/JWT exp) — окрема A.7 задача; qr_token лишається як є.
- Gift-at-checkout (мінт одразу на отримувача при купівлі) — суміжна checkout-задача; тут лише пост-покупковий transfer/gift.
- Wallet (.pkpass/Google Wallet), SEO-лендінги, admin finance/reconciliation dashboards.
- Guest claim-token генерація/зберігання/anti-abuse — власність INIT-TICKET-UNCLAIMED; тут — інтеграційний контракт виклику.
- ledger_entries append-only — TASK-LEDGER-1; тут пишемо коректні escrow-переходи, які ledger згодом віддзеркалить.

**Модель даних:**
- migration 0033_order_items.sql: order_items(id UUID PK, order_id UUID NOT NULL REFERENCES orders ON DELETE CASCADE, ticket_id UUID UNIQUE REFERENCES tickets(id), tier_id UUID NULL REFERENCES ticket_tiers(id), amount NUMERIC(12,2) NOT NULL, promo_share NUMERIC(12,2) NOT NULL DEFAULT 0, escrow_status TEXT NOT NULL DEFAULT 'held', refunded_amount NUMERIC(12,2) NOT NULL DEFAULT 0, created_at TIMESTAMPTZ DEFAULT now()); одна per-ticket частка escrow. INDEX (order_id), INDEX (ticket_id).
- Backfill: для кожного наявного orders з ticket_id — INSERT order_items(order_id, ticket_id, tier_id NULL, amount=orders.amount, escrow_status=orders.escrow_status). orders.escrow_status лишається агрегатом (derived = min-progress з items) для сумісності DTO.
- migration 0034_ticket_transfers.sql: ticket_transfers(id UUID PK, ticket_id UUID NOT NULL REFERENCES tickets(id) ON DELETE CASCADE, from_user_id UUID NOT NULL REFERENCES users(id), to_user_id UUID NULL REFERENCES users(id), to_email TEXT NULL, kind TEXT NOT NULL CHECK (kind IN ('gift','exchange')), created_at TIMESTAMPTZ DEFAULT now()); owner-history + лічильник transfer через COUNT.
- migration 0035_tickets_transfer_state.sql: tickets ALTER — прибрати логіку owner_id-flip; додати purchaser_id UUID NULL (первинний buyer, для аналітики), transferred_from UUID NULL REFERENCES tickets(id) (ланцюг подарунків). status CHECK розширити на ('active','used','returned','transferred','refunded','frozen'). Ослабити tickets_free_active_owner_event_uniq: транзитивно новий квиток при transfer вже active — індекс лишається бо старий стає transferred (не active).
- migration 0036_refund_policy.sql: events ADD COLUMN refund_policy JSONB NULL (NULL = використати дефолт); payment_settings ADD COLUMN default_refund_policy JSONB NOT NULL DEFAULT '{"windows":[{"hours_before_start":168,"percent":100,"fee_returned":true},{"hours_before_start":24,"percent":50,"fee_returned":false},{"hours_before_start":0,"percent":0,"fee_returned":false}]}'; payment_settings ADD max_transfers_per_ticket INT NOT NULL DEFAULT 3, transfer_cooldown_minutes INT NOT NULL DEFAULT 60.
- orders.escrow_status семантика: derived-агрегат — 'held' поки будь-який order_item held; 'refunded' коли всі items refunded; частковий → лишається 'held'/'clearing'. Real per-ticket істина — в order_items.

**API:**
- POST /api/tickets/:id/transfer {recipient: {email?|handle?}, message?, kind?='gift'} → 200 {ticket: TicketDto|null, claim_pending: bool}. Owner-only, ticket.status='active'. FOR UPDATE на квитку. Перевірки: не собі; recipient age-gate; transfer_count < max_transfers_per_ticket; last_transfer + cooldown < now; escrow не frozen. Якщо recipient має акаунт: old→transferred, mint new→active(owner=recipient), перенести order_item.ticket_id на новий, INSERT ticket_transfers. Якщо guest (email без акаунта): виклик INIT-TICKET-UNCLAIMED claim-seam → old→transferred, створити pending-claim із signed short-TTL token, повернути claim_pending=true.
- POST /api/tickets/claim {token} (делеговано в INIT-TICKET-UNCLAIMED; тут — контракт): валідний single-use token → mint new active-квиток на поточного (щойно зареєстрованого) user, перенести order_item, escrow-власність → цей user.
- POST /api/tickets/:id/return → 200 {refund_amount, fee_returned, escrow_status}. Owner-only, active. FOR UPDATE. Обчислити суму через refund_policy(event, now vs starts_at, order_item.escrow_status). tickets→returned; tier.sold-- (FOR UPDATE на tier); order_item.escrow_status→refunded, refunded_amount=computed; orders.escrow_status перерахувати. Через escrow seam (provider.refund у майбутньому; зараз Noop).
- POST /api/tickets/:id/exchange {target_tier_id} → 200 {ticket, top_up: {order_id, amount, provider_action}|null}. Helper: return поточного (за політикою) + checkout на target_tier; якщо дорожче — повертає order intent на доплату; якщо дешевше — різниця в refund. НЕ окремий стан.
- POST /api/tickets/validate {qr_token, event_id} → {valid, reason ∈ ok|already_used|transferred|returned|refunded|wrong_event|inactive, owner_name, tier_name, validated_at}. FOR UPDATE OF t. active→used; transferred/returned/refunded → valid=false з точним reason (закриває double-spend вікно).
- GET /api/me/tickets → TicketDto[] += owner_name, tier_name, transfer_count, is_gift(=purchaser_id≠owner_id), refundable(bool), refund_estimate(NUMERIC), escrow_status(per-ticket з order_item), segment ∈ upcoming|past|returned_gifted.
- POST /api/orders/:id/dispute {reason} — авторизація зміщується: дозволено ПОТОЧНОМУ owner будь-якого квитка ордера (через order_items→tickets.owner_id), не лише orders.buyer_id. Guest — після claim. freeze лише order_item(-и) поточного owner.

**UI:**
- Android ETicketScreen: кнопки Подарувати / Повернути / Обмін; badge статусу (transferred/returned/refunded/frozen); показ owner_name+tier; прибрати сирий Text(qrToken).
- Android GiftTicketDialog: поле email АБО @handle; message; підтвердження; показ 'залишилось N передач' та cooldown, якщо активний.
- Android RefundConfirmSheet: сума повернення + чи повертається збір + строк (прив'язка до вікна політики) + причина; чітке підтвердження перед незворотним.
- Android ExchangeSheet: вибір target tier, показ доплати/повернення різниці; один tap.
- Android MyTicketsScreen: сегменти Майбутні|Минулі|Повернені/Подаровані; міні-постер; мітка 'подарунок'; кнопка дарунка прямо на картці.
- Web /me/tickets/[id] (owner) full-ticket + /t/[token] (guest claim): 'Отримати квиток' → signup/login inline → claim; показ дарувальника+повідомлення ДО auth-wall (цінність перед авторизацією).
- Admin order detail drawer: per-ticket escrow breakdown (order_items), partial-refund історія, ланцюг transfer (ticket_transfers), поточний owner vs purchaser.

**Acceptance Criteria:**
- AC-1: Transfer активного квитка користувачу з акаунтом атомарно позначає старий 'transferred', мінтить новий 'active' на отримувача, переносить order_item.ticket_id, пише ticket_transfers; tier.sold і tickets_left НЕ змінюються (передача, не продаж).
- AC-2: Transfer на email без акаунта повертає claim_pending=true, старий квиток→'transferred', створює single-use short-TTL claim-token (через INIT-TICKET-UNCLAIMED); отримувач за лінком реєструється і claim мінтить активний квиток на нього.
- AC-3: N+1-й transfer одного квитка (N=max_transfers_per_ticket) відхиляється 409; transfer раніше cooldown після попереднього — 409 з чітким повідомленням.
- AC-4: Refund активного квитка обчислює суму СТРОГО за refund_policy(event||default) відносно (starts_at - now): у вікні 100%/fee_returned=true до старту повертається повна сума включно зі збором; у ступінчастих вікнах — відповідний %; після старту/за політикою 0 → 409.
- AC-5: Partial refund 1-з-N: return одного квитка з ордера на N квитків лишає решту N-1 'active', зменшує tier.sold рівно на 1, встановлює лише його order_item.escrow_status='refunded' і refunded_amount; orders.escrow_status лишається 'held' поки є неповернені held-items.
- AC-6: Escrow/dispute-власність слідує за поточним owner: після gift первинний buyer БІЛЬШЕ не бачить order у своїх tickets і не може відкрити dispute; поточний owner (або guest після claim) може.
- AC-7: Double-spend закрито: одночасні validate і transfer одного квитка серіалізуються через FOR UPDATE — рівно один успішний; якщо transfer виграв, validate повертає reason='transferred', valid=false; якщо validate виграв (used), transfer→409.
- AC-8: Exchange на дорожчий tier повертає order intent на доплату різниці і НЕ активує новий квиток до оплати; на дешевший — мінтить новий tier одразу і повертає різницю за політикою; жодного нового стану квитка поза transferred/active.
- AC-9: my_tickets DTO повертає коректні owner_name, tier_name, transfer_count, is_gift, refund_estimate і per-ticket escrow_status; сегментація upcoming|past|returned_gifted правильна.
- AC-10: Усі грошові мутації (refund, часткові) проходять єдиний escrow seam; під Noop/TestProvider — детерміновані; жоден рядок доменної логіки не гілкується на конкретного провайдера.
- AC-11: Telemetry: кожен transfer, gift_claimed, ticket_refunded(із сумою), exchange пишуть подію — virality/refund-correctness вимірювані без нового екрана.

**Security-вектори (SEC):**
- SEC-1: Скальпінг через безлімітний transfer — mitigated лімітом N/квиток + cooldown + owner-history в ticket_transfers для аудиту велосіті.
- SEC-2: Double-spend transfer↔scan (передати після/під час сканування або навпаки) — mitigated FOR UPDATE на рядку квитка в обох шляхах; серіалізація гарантує single-winner.
- SEC-3: Крадіжка refund гостем/чужим — refund/dispute авторизується строго по поточному tickets.owner_id (guest лише після успішного claim single-use token), не по email/token в запиті.
- SEC-4: Подвійний refund / over-refund у multi-ticket — order_item.escrow_status + refunded_amount під FOR UPDATE; ідемпотентно; Σ refunded_amount ≤ order_item.amount інваріант.
- SEC-5: Claim-token abuse (перебір/повторне використання/витік) — single-use, short-TTL, підписаний; rate-limit на claim endpoint; email/device-cap (успадковано з INIT-TICKET-UNCLAIMED anti-abuse умови готовності).
- SEC-6: Age-gate bypass через gift/transfer/claim — recipient (і claimer при claim) перевіряється meets_age проти event.age_rating перед мінтом активного квитка.
- SEC-7: Escrow-власність void через gift під диспутом — квиток з frozen order_item НЕ передається (409); gift не має воїдити pending refund.
- SEC-8: Race на tier.sold/tickets_left при паралельних return/exchange/checkout — усі беруть FOR UPDATE на tier/event рядку; sold ніколи не < 0, left ніколи не > capacity.
- SEC-9: Transfer квитка не свого/чужого ордера — owner-only + order_item.ticket_id прив'язка перевіряється в тій самій транзакції.
- SEC-10: refund_policy JSONB injection/невалідна структура — валідація схеми при записі (events.refund_policy) у create/update-event; runtime парсинг у typed struct, без any/unsafe-cast.

**Test Matrix:**
- [POS] transfer→акаунт: старий transferred, новий active у отримувача, order_item перенесено, tier.sold незмінний.
- [POS] transfer→guest email: claim_pending, token видано, claim після signup мінтить active.
- [POS] return у 100%-вікні: повна сума + збір повернено, tier.sold--, escrow_status item=refunded.
- [POS] partial refund 1-з-3: 2 лишились active, sold-1, orders.escrow_status='held'.
- [POS] exchange дешевше: новий tier active одразу + різниця повернена.
- [POS] validate active→used: valid=true, reason=ok.
- [NEG] transfer N+1: 409 ліміт.
- [NEG] transfer у cooldown: 409.
- [NEG] return після старту/поза політикою: 409, escrow незмінний.
- [NEG] transfer не-owner: 403/404.
- [NEG] transfer used/returned/frozen квитка: 409.
- [NEG] refund невалідної refund_policy JSONB при create-event: 422 валідація.
- [SEC] паралельні validate+transfer: рівно один виграє; другий reason='transferred' або 409.
- [SEC] guest claim повторно з тим же token: 409/410 (single-use).
- [SEC] dispute первинним buyer після gift: 403/404 (не поточний owner).
- [SEC] over-refund того ж item двічі: ідемпотентно, Σ refunded ≤ amount.
- [SEC] age-restricted transfer/claim неповнолітньому: 409 age-gate.
- [SEC] gift квитка з frozen order_item: 409.
- [SEC] exchange паралельно з return того ж квитка: серіалізовано FOR UPDATE, без подвійного sold-двіження.
- [SEC] claim-token rate-limit/email-cap перевищено: 429.

---

## SDD-STAFF-1a — Делеговане door-scan (STAFF-1a): event_staff + require_perm + event-scoped validate + інвайт-лінк «Контролер» + безпечний signed QR з серверним anti-passback і повними вердиктами

**Мета:** Дозволити організатору делегувати перевірку квитків на вході довіреним контролерам (door-staff) в один клік (share-link → приймаю → скан), замінити сирий qr_token у застосунку на короткоживучий signed stable token, і зробити валідацію на вході атомарно захищеною від подвійного входу (anti-passback, first-write-wins) з повним набором людиночитних вердиктів. Це P0-листок, що розблоковує фізичний вхід на подію; offline-черга, Door Mode continuous-UI та scan-audit винесені в окремі листки (STAFF-1b/1c/1d).

**Поверхні:** API, Android  ·  **Effort:** M  ·  **Залежності:** Наявний HS256 JWT seam (api/src/auth.rs — encode/decode, pinned Algorithm::HS256) переюзається для signed QR і staff-invite токенів, tickets.validate (api/src/tickets.rs:163) — рефактор з глобального на event-scoped; наявний FOR UPDATE OF t + single UPDATE вже дає основу для anti-passback, events.status (migrations/0001_init.sql, events.rs) — розширення на 'cancelled' спершу у docs/spec.md (spec-is-truth §2), Наявний rate-limit/anti-abuse seam для guest/gift-шляхів (принцип «frictionless не означає діряво») — переюзати для staff-invite, telemetry.log seam (api/src/telemetry.rs) — tracking-події scan_success/scan_reject/staff_invite_sent/staff_accepted (KPI: scan success rate, double-entry rate, частка подій з делегованим staff, door-throughput), Deep-link інфраструктура застосунку для /staff/invite/:token (спільна зі share/gift-петлею)

**У scope:**
- event_staff: нова таблиця (event_id, user_id, perms, invited_by, created_at) — many-to-many контролери на подію, perms=['scan'] за замовчуванням
- require_perm(event_id, user, perm): єдиний серверний guard, що дозволяє дію організатору події АБО активному запису в event_staff з відповідним perm
- Event-scoped validate: POST /api/events/:event_id/tickets/validate — заміняє глобальний /api/tickets/validate; авторизація через require_perm(scan); wrong_event визначається за неспівпадінням event_id у path і в квитка
- Signed stable QR token: серверна видача коротко-живучого signed токена (HS256, той самий seam що auth JWT, окремий claim-набір: tid, eid, exp) поверх стабільного ticket_id — сирий qr_token у DB НЕ змінюється і НЕ відображається у застосунку
- Атомарний anti-passback на сервері: перший успішний scan робить first-write-wins UPDATE ... WHERE status='active' RETURNING; повторний реплей того самого QR (навіть офлайн-збережений) отримує already_used. scanned_at/validated_at — лише метадані, не є ключем ідемпотентності
- Повний набір вердиктів: valid | already_used | wrong_event | refunded | expired | not_started | cancelled | not_found — кожен з code+колір+іконка+текст (non-color-redundant)
- Інвайт-лінк «Контролер»: POST /api/events/:id/staff/invite → signed single-use short-TTL staff-invite токен; GET/POST /api/staff/invite/:token → accept у 1 клік (auth-wall лише в момент accept); guest без акаунта реєструється тут же й одразу стає staff
- Прибрати refund з дверей: жодних грошових операцій у door-flow; escrow/refund недоступні контролеру
- Android: екран сканера квитків доступний організатору І контролеру; повноекранний вердикт-стан (колір+іконка+текст); екран «Мої події як контролер»; кнопка «Запросити контролера» → share-link
- event.status розширюється значенням 'cancelled' (spec-first у docs/spec.md) для вердикту cancelled

**Поза scope:**
- STAFF-1b: Door Mode continuous-loop UI (безперервне сканування без ручного тапу між квитками) — окремий P1-листок
- STAFF-1c: offline черга сканів + синк при відновленні мережі — P2
- STAFF-1d: детальний scan audit-лог (хто/коли/девайс/результат кожного скану) — P2
- TOTP/rotating-QR ротація секрету — відкладено (несумісна з offline; не в цьому листку)
- Refund/повернення/escrow-операції на дверях
- Гранульовані ролі понад perms=['scan'] (напр. 'refund','manage') — зарезервовано в схемі, але UI/логіка лише scan
- Веб-адмін UI для керування staff (тільки API + Android у цьому листку)

**Модель даних:**
- migration 0033_event_staff.sql: CREATE TABLE event_staff (id UUID PK DEFAULT gen_random_uuid(), event_id UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE, user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE, perms TEXT[] NOT NULL DEFAULT ARRAY['scan'], invited_by UUID REFERENCES users(id), created_at TIMESTAMPTZ NOT NULL DEFAULT now(), UNIQUE(event_id, user_id))
- migration 0033: CREATE INDEX event_staff_user_idx ON event_staff(user_id); CREATE INDEX event_staff_event_idx ON event_staff(event_id)
- tickets: додати scanned_at TIMESTAMPTZ (метадані останнього успішного скану; окремо від наявного validated_at — validated_at лишається для сумісності або мапиться на scanned_at); scanned_by UUID REFERENCES users(id) NULL
- events: розширити CHECK/дозволені значення status на 'cancelled' (0034_event_cancelled.sql) — використовується для вердикту cancelled; спершу зафіксувати у docs/spec.md
- Жодних нових таблиць для QR: signed token — stateless (не зберігається); anti-passback тримається на рядку tickets (status), а не на окремому ключі

**API:**
- POST /api/events/:event_id/tickets/validate — body {qr_token:string}; auth require_perm(event_id, scan); резолвить signed-token→ticket_id, перевіряє eid==event_id (інакше wrong_event); атомарно UPDATE tickets SET status='used', scanned_at=now(), scanned_by=$user WHERE id=$tid AND status='active' RETURNING; повертає {valid:bool, verdict:string, ticket_holder:string?, event_title:string, ticket_tier:string?}
- GET /api/tickets/:id/qr — власник отримує short-TTL signed QR token {token:string, exp} для показу (заміняє сирий qr_token у застосунку); TTL напр. 120s, стабільний tid всередині (реген лише продовжує exp, не міняє tid)
- POST /api/events/:id/staff/invite — auth=organizer; body {perms?:string[]=['scan']}; повертає {invite_url, token, exp} (single-use short-TTL signed staff-invite)
- POST /api/staff/invite/:token/accept — auth (auth-wall тут); валідує single-use short-TTL токен, upsert event_staff(event_id,current_user,perms), повертає {event_id, event_title, perms}; guest-шлях: якщо не залогінений — після signup цей же виклик
- GET /api/staff/invite/:token — public preview (без auth): {event_title, organizer_name, perms} для екрану прийняття до логіну (цінність перед авторизацією)
- GET /api/me/staff-events — список подій, де caller є контролером: [{event_id, title, starts_at, perms}]
- GET /api/events/:id/staff — auth=organizer; список контролерів події [{user_id, display_name, perms, created_at}]
- DELETE /api/events/:id/staff/:user_id — auth=organizer; відкликати контролера
- DEPRECATE POST /api/tickets/validate — лишити тимчасово як alias, що резолвить event з квитка і делегує на event-scoped guard (не ламати клієнтів), позначити deprecated; НЕ breaking

**UI:**
- Android «Сканер квитків» (organizer+staff): камера → зчитує signed QR → POST validate → повноекранний вердикт-стан. Кожен вердикт = колір + іконка + великий текст + підпис: valid(зелений,✓,«Дійсний»), already_used(жовтий,⟳,«Вже використаний»+час), wrong_event(помаранч,⚠,«Інша подія»), refunded(сірий,↩,«Повернений»), expired(сірий,⌛,«Прострочений QR»), not_started(синій,⏱,«Вхід ще не відкрито»), cancelled(червоний,✕,«Подію скасовано»), not_found(червоний,?,«Квиток не знайдено»). Кнопка «Наступний» повертає до камери (continuous — у 1b)
- Android «Показати квиток» (власник): рендерить signed QR (не сирий рядок), авто-оновлення до закінчення TTL; підпис «Оновлюється автоматично»
- Android «Запросити контролера» (organizer, екран події): кнопка → POST staff/invite → системний share-sheet з invite_url (месенджери/соц); список поточних контролерів з опцією «Прибрати»
- Android екран прийняття інвайту (deep-link /staff/invite/:token): показує event_title+organizer ДО логіну; CTA «Стати контролером» → якщо гість, frictionless signup у пару кліків → accept → перехід у «Мої події (контролер)»
- Android «Мої події як контролер»: список подій зі швидким входом у сканер

**Acceptance Criteria:**
- AC-1: Організатор події може створити інвайт-лінк контролера; перехід за ним і accept створює рядок event_staff з perms=['scan']; після цього контролер може успішно сканувати квитки саме цієї події
- AC-2: Контролер БЕЗ прав на іншу подію отримує 403 при спробі validate квитка чужої події (require_perm)
- AC-3: Перший scan активного квитка повертає verdict=valid і атомарно переводить status active→used; scanned_at/scanned_by заповнюються
- AC-4: Повторний scan того самого QR (у т.ч. збереженого/реплейнутого офлайн-клієнтом) повертає already_used і НЕ впускає вдруге (first-write-wins, без гонок при паралельних скан-запитах)
- AC-5: Scan квитка, чий eid не дорівнює :event_id у path, повертає wrong_event і НЕ змінює статус
- AC-6: Кожен з вердиктів valid/already_used/wrong_event/refunded/expired/not_started/cancelled/not_found повертається сервером з машинним code і рендериться у застосунку з унікальним кольором+іконкою+текстом (non-color-redundant)
- AC-7: Signed QR токен прострочений (exp минув) дає expired; квиток лишається active і придатний після повторної видачі власником
- AC-8: У застосунку НІДЕ не показується і не логується сирий qr_token; на екрані власника і у скані фігурує лише signed token
- AC-9: Гість без акаунта, що перейшов за staff-інвайтом, бачить назву події й організатора ДО логіну, реєструється в пару кліків і одразу стає контролером (auth-wall лише на accept)
- AC-10: На door-flow недоступні жодні refund/escrow-операції (немає ендпоінтів/кнопок)
- AC-11: staff-invite токен single-use і short-TTL: повторний accept тим самим токеном або після TTL відхиляється
- AC-12: Організатор бачить список контролерів події і може відкликати будь-кого (DELETE); після відкликання validate від нього дає 403
- AC-13: Подія зі status='cancelled' дає вердикт cancelled для всіх її квитків при скані

**Security-вектори (SEC):**
- SEC-1: Anti-passback (double-entry): атомарний first-write-wins UPDATE ... WHERE status='active' у транзакції гарантує рівно один valid навіть при паралельних/офлайн-реплей скан-запитах; scanned_at НЕ є ключем ідемпотентності
- SEC-2: Витік секрету через QR: у застосунку показується короткоживучий signed token, а не сирий qr_token; сирий токен не залишає сервер у відповідях, призначених UI сканера/власника
- SEC-3: Privilege escalation: require_perm суворо перевіряє organizer_id ПОДІЇ або активний event_staff з perm=scan САМЕ для цього event_id; ніякого глобального scan-права
- SEC-4: Cross-event replay: eid всередині signed token + event-scoped ендпоінт унеможливлюють використання валідного квитка однієї події на дверях іншої (wrong_event)
- SEC-5: Signed-token forgery/algorithm-substitution: HS256 pinned на encode+decode (як у auth.rs), відхилення інших alg; невалідний підпис → not_found/expired, ніколи не valid
- SEC-6: Staff-invite abuse: single-use + short-TTL + rate-limit на /staff/invite і /staff/invite/:token/accept; device/email-cap на guest-signup з інвайту (anti-abuse обгортка як умова готовності)
- SEC-7: Token replay after revoke: відкликаний контролер (DELETE event_staff) миттєво втрачає доступ — require_perm читає актуальний рядок, кешованих прав немає
- SEC-8: Refund-at-door видалено — контролер не може повернути/заморозити гроші через door API (мінімізація повноважень)

**Test Matrix:**
- POS: organizer invite → accept → staff scan active ticket → valid + status→used
- POS: valid → already_used на другому скані
- POS: expired signed token → expired, квиток лишається active; повторна видача → valid
- POS: not_started (now < event.doors/starts_at) → not_started, статус не змінюється
- POS: cancelled event → cancelled вердикт
- POS: refunded/returned ticket → refunded вердикт
- POS: guest via invite → preview без auth → signup → accept → може сканувати
- POS: organizer бачить staff-list, DELETE відкликає
- NEG: validate квитка чужої події → wrong_event (статус не змінено)
- NEG: невідомий/підроблений token → not_found (не valid)
- NEG: accept простроченого/уже-використаного invite → 4xx
- NEG: не-organizer викликає /staff/invite → 403
- [SEC] паралельні N validate одного active QR → рівно один valid, решта already_used (гонка/anti-passback)
- [SEC] offline-replay: клієнт повторно шле раніше валідний token після виходу з мережі → already_used, не входить вдруге
- [SEC] token підписаний іншим alg/секретом → відхилено, не valid (algorithm-substitution)
- [SEC] відкликаний staff намагається сканувати → 403
- [SEC] rate-limit/email-cap на масовий guest-signup з одного staff-інвайту
- [SEC] сирий qr_token не присутній у жодній відповіді, яку споживає UI (контрактний тест на форму відповіді)

---

## SDD-018 — Органічний пошук: SSR-афіша + JSON-LD + гео/категорійні лендінги (INIT-SEO)

**Мета:** Увімкнути органічний пошуковий трафік як дешевий, паралельний до месенджер-шерингу канал росту non-paid користувачів: зробити головну афішу індексованою (SSR-список подій замість 'use client'), додати JSON-LD (Event на сторінку події + ItemList на лендінги/афішу), і створити canonical long-tail лендінги /c/[category] та /city/[slug] (Київ-first: райони/майданчики). Кожна нова сторінка веде відвідувача до перегляду події та дії (RSVP/claim/buy) — ціннісний контент видно ДО логіну. Рішення тримається простим: жодного нового бек-офісу, лише тонкий SSR-шар поверх наявних публічних endpoint-ів + один новий read-only endpoint для фасетів.

**Поверхні:** Web (Next.js App Router SSR/ISR), API (Rust/Axum — 1 новий read-only endpoint), MKT (sitemap/robots/JSON-LD/canonical)  ·  **Effort:** M  ·  **Залежності:** Наявні публічні endpoint-и GET /api/events(?category=&city=&q=&sort=&past=) та GET /api/events/:id (готові)., Наявний SEO-фундамент: web/app/sitemap.ts, robots.ts, layout.tsx metadataBase/OG (готові)., Наявний telemetry seam POST /api/telemetry + logTelemetry у web/lib/api.ts., Наявний EventCard та lib/i18n (для двомовних інтро)., NEXT_PUBLIC_SITE_URL та NEXT_PUBLIC_API_BASE_URL env (готові)., Не блокує і не блокується месенджер-шеринг/gift-петлею (паралельний трек, не конкурує за slot).

**У scope:**
- Прибрати 'use client' з web/app/page.tsx: перетворити головну афішу на Server Component, що SSR-рендерить список опублікованих подій через fetch(GET /api/events) на сервері; інтерактив (save-toggle, вкладки For you/All, клієнтський пошук) винести в дочірній Client Component, що гідратується поверх SSR-розмітки.
- Додати JSON-LD schema.org Event (@type Event) як <script type="application/ld+json"> у SSR-розмітку сторінки події web/app/events/[eventId]/page.tsx — рендериться на сервері з initialEvent (не всередині 'use client' EventDetailClient).
- Додати JSON-LD ItemList на головну афішу та на лендінги (перелік подій з url/position/name).
- Новий route web/app/c/[category]/page.tsx — SSR категорійний лендінг: H1 + інтро-копірайт + SSR-список подій цієї категорії (fetch GET /api/events?category=), canonical URL, ItemList JSON-LD, generateMetadata (title/description/OG).
- Новий route web/app/city/[slug]/page.tsx — SSR гео-лендінг: H1 + інтро + SSR-список подій міста (fetch GET /api/events?city=), canonical, ItemList JSON-LD, generateMetadata. Київ-first: підтримка slug-ів райони/майданчики через статичну мапу slug→{city, filter} (без нової БД-таблиці).
- Новий read-only endpoint GET /api/facets (api/src/events.rs або новий facets.rs) — повертає список distinct published-категорій та distinct міст із лічильниками, для генерації статичних лендінг-роутів (generateStaticParams) і sitemap.
- Розширити web/app/sitemap.ts: додати URL-и всіх лендінгів /c/[category] і /city/[slug] (з /api/facets).
- Оновити robots.ts: дозволити /c/ та /city/ (вони вже під allow "/" — підтвердити, що не потрапляють у disallow).
- generateStaticParams + revalidate (ISR) для лендінгів і головної, щоб сторінки були статично закешовані та періодично оновлювались (revalidate=3600).
- Telemetry-події перегляду лендінгів (landing_impression з props: kind=category|city, slug) через наявний /api/telemetry seam.
- i18n: інтро-копірайт лендінгів двомовний (UA-first, EN) через наявний lib/i18n; slug-и — латиниця/транслітерація, стабільні.

**Поза scope:**
- Курсор-пагінація параметризованого пошуку (окрема P0-задача discovery; тут лендінги рендерять перший екран подій без нескінченного скролу).
- Нові фільтри date_from/date_to/price_max на бекенді (не потрібні для SEO-лендінгів v1).
- Повнотекстовий пошук/Elasticsearch — лишаємо наявний ILIKE.
- Динамічне SEO для сторінок організаторів/користувачів.
- Багатомовні (hreflang) дублі URL-ів — v1 один canonical UA-контент з EN-переключенням на клієнті.
- Реальні платежі/escrow-зміни — payment seam не чіпаємо.
- Створення БД-таблиці міст/районів/майданчиків з координатами (використовуємо статичну slug-мапу + наявні city-рядки подій).

**Модель даних:**
- Без нових таблиць та міграцій — усе поверх наявних events(status,category,city,starts_at,...).
- GET /api/facets читає з events: SELECT category, count(*) ... WHERE status='published' GROUP BY category; аналогічно city (WHERE city IS NOT NULL).
- Статична мапа гео-slug-ів у web (web/lib/geo.ts): SLUG→{ city: string; label_uk; label_en; district?/venue?: filter-hint }. Київ-first, розширювана без міграцій.
- Статична мапа категорій-slug у web (web/lib/categories.ts): SLUG↔category-рядок + label_uk/label_en (наявні category-рядки подій — концерти/вечірки/події).
- Індекс-рекомендація (не обовʼязково для v1, лише якщо facets повільний): CREATE INDEX idx_events_status_category ON events(status, category); idx_events_status_city ON events(status, city) — за потреби у наступній ітерації, поза цим scope якщо не потрібно.

**API:**
- GET /api/facets → 200 { categories: [{ category: string, count: number }], cities: [{ city: string, count: number }] } — публічний, no-auth, лише published, count>0. Кеш-заголовки (Cache-Control: public, max-age=3600).
- GET /api/events?category=<c> → існує, використовується SSR-лендінгом /c/[category] (published, upcoming за замовч.).
- GET /api/events?city=<c> → існує, використовується SSR-лендінгом /city/[slug].
- GET /api/events → існує, використовується SSR головної афіші (заміна клієнтського fetch).
- GET /api/events/:id → існує, вже SSR у page.tsx; додаємо JSON-LD у розмітку.
- POST /api/telemetry → існує; нова подія name="landing_impression", props={ kind, slug, count }.

**UI:**
- / (головна афіша, SSR): H1 «Афіша подій», SSR-грід EventCard з першою сторінкою подій (видно без JS/логіну), ItemList JSON-LD; клієнтський острівець — вкладки For you/All, save-toggle, пошук/фільтри. Порожній стан — SSR-текст.
- /events/[eventId] (вже SSR): додати Event JSON-LD (name, startDate=starts_at, location=venue_address/city, image=poster_url, offers={price,priceCurrency,availability}, eventStatus, description, url=canonical).
- /c/[category] (новий, SSR): H1 «<Категорія> — події», інтро-параграф (1-2 речення, двомовний), SSR-грід подій категорії, ItemList JSON-LD, canonical, крос-лінки на суміжні лендінги (інші категорії/міста) для внутрішньої перелінковки. Порожній стан + noindex якщо 0 подій.
- /city/[slug] (новий, SSR): H1 «Події у <Місто/Район>», інтро, SSR-грід подій міста, ItemList JSON-LD, canonical, крос-лінки на категорії в цьому місті. Київ-first slug-и (kyiv, kyiv-podil, kyiv-obolon тощо). Порожній стан + noindex якщо 0 подій.
- Внутрішня перелінковка: на головній та у футері/секції — лінки на топ-лендінги (з /api/facets), щоб краулер їх знаходив (не лише через sitemap).

**Acceptance Criteria:**
- AC-1: GET / повертає повний HTML зі списком опублікованих подій у markup-і при вимкненому JS (curl без JS показує заголовки/лінки подій); 'use client' відсутній у web/app/page.tsx (сторінка — Server Component).
- AC-2: HTML сторінки /events/[eventId] містить рівно один <script type="application/ld+json"> з валідним @type=Event (name, startDate, location, offers.price, offers.priceCurrency, url=canonical), рендериться на сервері й проходить Google Rich Results / schema.org валідацію.
- AC-3: /c/[category] для наявної категорії повертає 200 SSR-сторінку з H1, canonical=<SITE>/c/[category], ItemList JSON-LD з ≥1 елементом, list подій точно = GET /api/events?category=. Невідома категорія → 404.
- AC-4: /city/[slug] для відомого slug (kyiv) повертає 200 SSR з H1, canonical, ItemList; невідомий slug → 404; slug стабільний і латиничний.
- AC-5: GET /api/facets повертає лише published-категорії/міста з count>0; порожній масив коли подій немає; публічний без auth.
- AC-6: sitemap.xml містить URL-и всіх непорожніх /c/[category] та /city/[slug] додатково до / та /events/*.
- AC-7: robots.txt дозволяє /c/ і /city/; /me/, /organizer/, /login, /register лишаються disallow.
- AC-8: Лендінг з 0 подій рендерить <meta name="robots" content="noindex"> (не засмічує індекс тонким контентом).
- AC-9: Save-toggle, вкладки For you/All та клієнтський пошук на головній працюють після гідратації так само, як до рефактору (жодної регресії функціоналу).
- AC-10: landing_impression telemetry-подія відсилається при відкритті /c/* та /city/* з коректними props (kind, slug, count).
- AC-11: Головна та лендінги використовують ISR (revalidate=3600); сторінка події revalidate=60 (як зараз).

**Security-вектори (SEC):**
- SEC-1: GET /api/facets та всі SSR-fetch показують ЛИШЕ status='published' події — draft/cancelled/приватні дані не витікають у публічний HTML/JSON-LD/sitemap.
- SEC-2: [category]/[slug] параметри не інжектяться в SQL — фільтр city/category йде через параметризований bind у наявному list_events (вже параметризовано); web-шар лише валідує slug проти статичної allow-мапи перед fetch.
- SEC-3: JSON-LD і H1/інтро екрануються (XSS): title/description/city подій виводяться через React (авто-екранування); у JSON-LD використати JSON.stringify (не інтерполяцію рядків у <script>), захист від </script>-breakout.
- SEC-4: canonical/OG url будуються з trusted SITE env + валідованого slug — жодного host/redirect із user-input.
- SEC-5: /api/facets rate-limit/кеш (Cache-Control max-age=3600) щоб уникнути дешевого DB-навантаження від краулерів/скану.
- SEC-6: noindex на порожніх/тонких лендінгах не відкриває приховані фільтри як індексовану поверхню для scraping-а неопублікованого.
- SEC-7: robots disallow приватних маршрутів не послаблюється; нові allow обмежені саме /c/ і /city/.

**Test Matrix:**
- [positive] GET / (JS off) → HTML містить ≥1 заголовок опублікованої події та лінк /events/<id>.
- [positive] GET /events/[eventId] → валідний Event JSON-LD, всі обовʼязкові поля присутні; schema-валідатор зелений.
- [positive] GET /c/<known> → 200, canonical коректний, ItemList = list events?category.
- [positive] GET /city/kyiv → 200, H1 «Події у Києві», ItemList непорожній.
- [positive] GET /api/facets → лише published, count-и співпадають з фактичними опублікованими подіями.
- [positive] sitemap.xml → містить /c/* і /city/* для непорожніх фасетів.
- [negative] GET /c/<unknown-category> → 404.
- [negative] GET /city/<unknown-slug> → 404.
- [negative] Лендінг з 0 подій → 200 з noindex-meta, без битого ItemList.
- [negative] API недоступний під час SSR → сторінка деградує (порожній стан), не 500 (як у наявному sitemap try/catch).
- [SEC] draft/cancelled подія НЕ зʼявляється у /, /c/*, /city/*, /api/facets, sitemap, JSON-LD.
- [SEC] slug/category з SQL-метасимволами ('; DROP…, %, _, \) → 404 (не в allow-мапі) або безпечний параметризований запит без інжекції.
- [SEC] подія з title/description що містить </script> або <img onerror> → у JSON-LD екранована (JSON.stringify), у HTML екранована React; XSS не спрацьовує.
- [SEC] спроба host-інʼєкції у canonical через заголовки → canonical завжди з SITE env.
- [regression] save/unsave, For you/All, клієнтський пошук на головній працюють після гідратації.

---

## SDD-LIFECYCLE-TRIGGERS — Тонкий lifecycle-шар: 5 named-тригерів + єдиний throttle-gate поверх наявного outbox

**Мета:** Підняти retention (D7/D30), review-conversion і checkout-completion, не перетворюючи продукт на важкий CRM: 5 named-тригерів (event-reminder T-24h, друзі-йдуть, review-nudge T+год, abandoned-checkout T+1h, scarcity) поверх наявного notifications/push outbox, з ОДНИМ throttle-gate (quiet-hours + frequency-cap + dedup) перед доставкою, і денним соц-дайджестом замість пуш-на-RSVP — щоб не генерувати spam-берст (uninstall-driver). Кожен тригер веде до north-star (квитки/актив.користувач) або віральності (друзі-йдуть, share-deep-link).

**Поверхні:** API (Rust/Axum): нові sweep-функції + throttle-gate + міграції, Android (Compose): deep-link routing з push payload + екран Notification Settings, MKT: copy/локалізація (uk/en) 5 тригерів, UX: quiet-hours/frequency defaults, settings-екран  ·  **Effort:** M  ·  **Залежності:** INIT-CHANNELS — push transport (FCM вже є в push.rs; додаткові канали SMS/Viber опційні, не блокують push+in-app MVP)., INIT-ANALYTICS — telemetry seam вже є (telemetry.rs); потрібні лише нові event-назви (дешеве розширення, не блокер)., Наявний код: reminders.rs (розширюємо), push.rs (payload+category), notifications.rs (category-колонка), orders.rs (abandoned читає), reviews.rs (nudge gate), user_prefs (opt-out+quiet), main.rs (нові workers)., event_timezone контракт — потрібен TZ на подію/юзера для quiet-hours; якщо ще нема, фолбек на фіксований Europe/Kyiv (UA-ринок) до появи поля., docs/spec.md — ДОДАТИ поведінку lifecycle-тригерів + throttle як single source of truth ПЕРЕД кодом (rules §0/§2).

**У scope:**
- 5 named-тригерів як окремі sweep-функції поверх наявного `notifications` outbox: event_reminder (T-24h вже частково є в reminders.rs — розширюємо), friends_going (агрегація 'N друзів йдуть' + денний cap), review_nudge (T+1..T+24h після ends_at для verified-attendee, deep-link на post_review), abandoned_checkout (order status='pending' старше 1h без paid, deep-link у той самий checkout), scarcity (tickets_left < N АБО sales_end наближається).
- Єдиний throttle-gate `api/src/lifecycle.rs::gate()`, який стоїть МІЖ вставкою notification і push-доставкою (push::pending): quiet-hours (22:00-08:00 у event_timezone/локальному TZ юзера), frequency-cap (max K lifecycle-пушів/добу на юзера), dedup (не слати той самий (kind,event) двічі — вже є event_reminders ledger, узагальнюємо).
- Соц-пуші (like/comment/follow/RSVP-друга) переводимо на ДЕННИЙ дайджест: один агрегований push 'N нових подій у ваших друзів' раз/добу, замість push-на-кожен-RSVP. In-app notifications лишаються реального часу.
- Per-user opt-out на рівні категорії тригера (reminders/social/marketing) через розширення user_prefs; auth-wall не потрібен — усі тригери таргетять лише зареєстрованих юзерів з подіями/квитками.
- Telemetry-події для кожного тригера (fired/delivered/suppressed/opened/converted) через наявний telemetry::log seam.
- Android: обробка deep-link payload у push (kind + event_id + order_id) → навігація на EventDetail / Checkout / ReviewSheet; екран налаштувань сповіщень (3 тумблери + opt-out).

**Поза scope:**
- Важкий CRM / сегментація / A-B движок / journey-builder — свідомо НІ (принцип 'не другий Instagram/Braze').
- Email-канал для lifecycle (лишаємо тільки наявний order-confirmation email; lifecycle — тільки push+in-app у цій ітерації).
- Гостьові/анонімні тригери (guest gift/share loop) — це окрема ініціатива INIT-CHANNELS; тут не чіпаємо.
- Нові провайдери платежів / зміна escrow / payout — payment seam лишається as-is (provider-agnostic, не торкаємось).
- Machine-learning send-time optimization — фіксований T-24h/T+1h достатньо для MVP.
- SMS/Viber/Telegram transport — залежність INIT-CHANNELS, не тут.

**Модель даних:**
- MIGRATION 0033_lifecycle.sql — узагальнити dedup-ledger: перейменувати логіку `event_reminders` → загальна таблиця `lifecycle_sends (user_id UUID, kind TEXT, event_id UUID NULL, order_id UUID NULL, sent_at TIMESTAMPTZ DEFAULT now(), PRIMARY KEY(user_id, kind, COALESCE(event_id,'00..0'), COALESCE(order_id,'00..0')))` — один рядок = один спрацьований тригер, гарантує dedup через UPSERT ON CONFLICT DO NOTHING. Зберегти `event_reminders` як є або мігрувати дані в неї.
- MIGRATION 0033: розширити `user_prefs` трьома BOOLEAN NOT NULL DEFAULT true: `notify_reminders`, `notify_social`, `notify_marketing` (opt-out per категорія); + `quiet_hours_start SMALLINT DEFAULT 22`, `quiet_hours_end SMALLINT DEFAULT 8` (година в TZ юзера).
- MIGRATION 0033: `notifications` дістає `category TEXT NOT NULL DEFAULT 'social'` (reminders|social|marketing) — керує throttle-gate і opt-out; та `dedup_key TEXT NULL` (для узагальненого dedup, опційно).
- Використати наявні поля: `orders.created_at`+`status='pending'` (abandoned-checkout), `orders.escrow_status` не чіпати, `events.starts_at`/`ends_at`/`tickets_left`/`sales_end` (reminder/scarcity), `reviews (event_id,author_id)` unique (review_nudge — слати лише якщо нема рядка), `tickets.owner_id`+`status IN('active','used')` (verified-attendee gate для review_nudge), `follows`+`tickets`/`event_saves` (friends_going).
- Лічильник frequency-cap НЕ нова таблиця: `SELECT count(*) FROM notifications WHERE user_id=$1 AND category IN ('reminders','marketing') AND created_at >= now()-interval '24 hours'` перед вставкою.

**API:**
- POST /api/orders/:id/pay-test та /api/events/:id/checkout — БЕЗ змін контракту; abandoned-checkout читає orders напряму (жодного нового endpoint для тригера — sweep-и це internal worker-логіка).
- GET /api/me/notification-settings → 200 {notify_reminders:bool, notify_social:bool, notify_marketing:bool, quiet_hours_start:int(0-23), quiet_hours_end:int(0-23)} (читає user_prefs).
- PUT /api/me/notification-settings body {notify_reminders?:bool, notify_social?:bool, notify_marketing?:bool, quiet_hours_start?:int, quiet_hours_end?:int} → 200 з повним станом; валідація 0<=hour<=23 інакше 422.
- Розширити push payload (push::PendingPush→FCM data): додати поля `category`, `order_id` (для abandoned-checkout deep-link у той самий checkout) поряд з наявними kind/event_id. Deep-link URI-схема: `underground://event/{event_id}`, `underground://checkout/{order_id}`, `underground://event/{event_id}/review`.
- Internal (не HTTP): `lifecycle::sweep_reminders`, `sweep_friends_going`, `sweep_review_nudge`, `sweep_abandoned_checkout`, `sweep_scarcity`, `sweep_social_digest` — кожен викликається зі свого tokio-worker у main.rs з окремим інтервалом; кожен ПЕРЕД вставкою notification викликає `lifecycle::gate(db,user_id,category,kind,event_id,order_id)`.
- `lifecycle::gate()` повертає bool allow: перевіряє (1) per-category opt-out у user_prefs, (2) dedup через lifecycle_sends UPSERT, (3) quiet-hours (поточна година в TZ юзера в [start,end)→відкласти/skip), (4) frequency-cap за останні 24h. Тільки якщо всі OK — вставляє notification + lifecycle_sends рядок.

**UI:**
- Android EventDetail: без нового екрана — reminder/scarcity/friends_going push відкриває наявний EventDetail через deep-link underground://event/{id}.
- Android Checkout: abandoned-checkout push відкриває наявний Checkout-екран з prefill order_id (underground://checkout/{order_id}) — той самий стан, не новий.
- Android ReviewSheet: review_nudge push → underground://event/{id}/review відкриває наявний bottom-sheet post_review (rating 1-5 + text).
- Android NEW: Settings → 'Сповіщення' екран: 3 тумблери (Нагадування про події / Активність друзів / Новини та пропозиції) + два time-picker (Не турбувати з / до). Дефолт: усі ON, quiet 22:00-08:00. Зберігає через PUT /api/me/notification-settings.
- Соц-дайджест push (uk): 'У ваших друзів N нових подій цього тижня' → відкриває стрічку друзів/Афішу.
- Copy (uk/en) кожного тригера — узгодити з MKT; локаль з user_prefs.locale як у наявному коді.

**Acceptance Criteria:**
- AC-1: event_reminder спрацьовує рівно раз на (event,user) для going+interested, коли starts_at у вікні [now, now+24h); повторний sweep НЕ дублює (lifecycle_sends/event_reminders dedup).
- AC-2: review_nudge шлеться лише verified-attendee (є tickets рядок owner_id зі status active/used) через 1-24h після ends_at, і ЛИШЕ якщо в reviews нема (event_id,author_id) — після залишення відгуку більше не нагадує.
- AC-3: abandoned_checkout шлеться для orders status='pending' старших за 1h, у яких нема paid-переходу; deep-link веде на checkout цього order_id; після оплати/скасування тригер не повторюється.
- AC-4: friends_going агрегує 'N друзів йдуть' (не N окремих пушів) з денним cap ≤1 push/подію/добу на юзера.
- AC-5: scarcity шле максимум один push/подію/юзера, коли tickets_left < N (конфіг, дефолт 10) АБО sales_end у вікні < 24h; не шле для free/минулих/sales_closed подій.
- AC-6: throttle-gate дотримує quiet-hours у TZ юзера — жоден lifecycle push не доставляється у [quiet_start, quiet_end); повідомлення відкладається до кінця quiet-вікна (in-app лишається).
- AC-7: frequency-cap: не більше K (дефолт 3) reminders+marketing пушів на юзера за 24h; понад cap — suppressed (лог telemetry lifecycle_suppressed).
- AC-8: per-category opt-out поважається: notify_reminders=false прибирає reminder+scarcity+abandoned; notify_social=false прибирає соц-дайджест+friends_going; notify_marketing=false прибирає scarcity-промо.
- AC-9: соц-активність (like/comment/follow/friend-RSVP) НЕ генерує миттєвий push — лише in-app notification + один денний дайджест-push.
- AC-10: PUT/GET /api/me/notification-settings персистять і повертають стан; невалідна година → 422.
- AC-11: кожен тригер пише telemetry: {trigger}_fired, _suppressed(reason), і при відкритті клієнтом _opened, _converted (напр. abandoned→paid).
- AC-12: усі workers ідемпотентні й не течуть: повторний запуск/перезапуск процесу не дублює доставки (watermark pushed_at + lifecycle_sends).

**Security-вектори (SEC):**
- SEC-1: notification-таргетинг НЕ розкриває приватних даних — friends_going шле лише про друзів (follows двобічно/односторонньо як у наявному eventsocial), blocked-юзери виключені (u.blocked=false) у всіх агрегаціях.
- SEC-2: abandoned-checkout deep-link на order_id ДОСТУПНИЙ лише buyer_id власнику; push шлеться лише на device_tokens цього user_id (наявний push fan-out по user_id) — чужий order недосяжний.
- SEC-3: review_nudge не можна використати для форсування фейк-відгуку — gate вже вимагає verified attendee (tickets) + reviews open лише після starts_at (наявна перевірка post_review).
- SEC-4: PUT /api/me/notification-settings авторизований CurrentUser, мутує лише свій user_prefs; неможливо змінити чужі налаштування.
- SEC-5: throttle/opt-out не обходиться — gate централізований, усі 5 тригерів ЗОБОВ'ЯЗАНІ проходити через lifecycle::gate() перед вставкою (жодного прямого INSERT notifications у нових sweep-ах).
- SEC-6: анти-spam frequency-cap сам є захистом від self-inflicted push-flood при масовій публікації подій організатором (uninstall-driver).
- SEC-7: quiet-hours обчислення TZ бере довірене джерело (event_timezone/user_prefs), не клієнтський заголовок — неможливо підмінити для обходу.
- SEC-8: telemetry props не логують PII (лише ids), як у наявному telemetry seam.

**Test Matrix:**
- [POS] reminder: подія за 12h, going+interested юзери → рівно по 1 notification, category=reminders.
- [NEG] reminder: повторний sweep у тому ж вікні → 0 нових (dedup).
- [NEG] reminder: подія за 48h або в минулому → 0.
- [POS] review_nudge: attendee з квитком, ends_at 2h тому, нема review → 1 nudge з deep-link review.
- [NEG] review_nudge: юзер без квитка / вже залишив review / подія ще не почалась → 0.
- [POS] abandoned: order pending 90хв, не оплачений → 1 push на buyer з checkout deep-link.
- [NEG] abandoned: order оплачений(paid) або <1h або скасований → 0; повторний sweep після push → 0.
- [POS] friends_going: 3 друзі going → 1 агрегований 'N друзів' push (не 3).
- [NEG] friends_going: другий sweep того ж дня → 0 (денний cap).
- [POS] scarcity: tickets_left=5 (<10) → 1 push; [NEG] free-подія / sales_closed / tickets_left=50 → 0.
- [POS/SEC] quiet-hours: тригер о 23:00 TZ юзера → push відкладено, не доставлено в quiet-вікні; in-app присутній.
- [POS/SEC] frequency-cap: 4-й reminder за добу при cap=3 → suppressed + telemetry lifecycle_suppressed(reason=freq_cap).
- [POS] opt-out: notify_reminders=false → reminder/scarcity/abandoned suppressed; notify_social=false → digest suppressed.
- [NEG/SEC] settings: PUT quiet_hours_start=25 → 422; PUT іншим токеном не змінює чужі prefs.
- [SEC] abandoned deep-link чужого order_id недосяжний іншому юзеру (push тільки на власника).
- [POS] social-digest: 5 friend-RSVP за день → 0 миттєвих пушів + 1 денний дайджест; in-app 5 notifications присутні.
- [POS] телеметрія: кожен тригер лишає _fired; abandoned→paid лишає _converted.
- [POS] ідемпотентність: рестарт процесу під час sweep → жодних дублів доставки.

---

## SDD-COLDSTART — Cold-start & supply-seeding: import подій з посилання, non-friend соц-доказ, liquidity launch-gate

**Мета:** Дати команді засіяти порожній Київ supply-first і уникнути ghost-town: (1) швидкий semi-manual import київських афіш у чернетки подій із посилання/тексту (concierge-онбординг організаторів на FREE/RSVP 0%); (2) non-friend соц-доказ у картці/деталях («N з Києва йдуть», «популярне поблизу», «організатора стежать X»), щоб discovery мав ознаки життя навіть із порожнім соц-графом; (3) liquidity launch-gate — адмін-дашборд із метриками ліквідності (події/тиждень у гео, активні організатори, search-success-rate) як явна умова public-запуску. Усе на free/RSVP + NoopProvider, БЕЗ реального PSP. Простота понад усе — жодних нових важких сутностей.

**Поверхні:** API (Rust/Axum): import endpoint, discover/social поля, admin liquidity, search telemetry, Admin (Next.js): /liquidity дашборд + бейдж imported у events, Web: EventCard non-friend соц-доказ рядок; import-форма в /organizer, Android: EventCard соц-доказ рядок (Compose) — read-only рендер нових полів  ·  **Effort:** M  ·  **Залежності:** Наявний telemetry_events (0016) + telemetry::log — переюзати, не створювати новий seam., Наявний cities-gazetteer + user_prefs.city/geo (0014/0015) — для going_city_count/nearby_popular/radius., Наявний discover.rs (DiscoverEvent) + eventsocial.rs (EventSocial) — точки розширення полів., Параметризований GET /api/events (search+фільтри) — TASK-DISCOVER-1: цей TZ додає лише telemetry-інструментацію 'search'; якщо параметризований пошук ще не злитий, search-логування чіпляється до нього як залежність., NoopProvider/free-RSVP шлях (директива №1) — 0% fee для seeded організаторів досягається price=0, БЕЗ нового fee-механізму., payment_settings або config-таблиця для порогів launch-gate (0033 додає рядки або окрему liquidity_config).

**У scope:**
- Import події з посилання/тексту: POST /api/organizer/events/import {source_url?, raw_text?} → чернетка (status=draft) з prefilled полями (title, description, starts_at, city, venue_address, poster_url, links, source_url) на review; owner редагує та публікує наявним POST/PUT /api/events. Парсинг — semi-manual (best-effort з тексту/OG-метаданих), НЕ автоскрейп із авторизацією за paywall.
- events.source (TEXT NULL: 'native'|'import') + events.source_url (TEXT NULL) + events.imported_by (UUID NULL) для трасування засіяних подій; бейдж «Афіша» на imported-чернетках у кабінеті організатора.
- Non-friend соц-доказ у discover/EventCard/Деталях: розширити GET /api/discover та GET /api/events/:id/social полями going_city_count (N активних квитків від користувачів того ж міста), nearby_popular (bool: подія в топ-K за going у радіусі), organizer_followers_count. EventCard рендерить рядок соц-доказу з fallback-пріоритетом: friends_going → going_city_count → nearby_popular → organizer_followers_count.
- Liquidity launch-gate дашборд: GET /api/admin/liquidity?city=&days=7 → {live_events_per_week, active_organizers, search_success_rate, gate_status} + пороги з payment_settings/config; адмін-екран /liquidity з світлофором PASS/FAIL по кожному критерію.
- search_success_rate: інструментувати параметризований пошук telemetry-подіями 'search' {city, has_query, result_count, zero_result:bool}; метрика = 1 - (zero-result searches / total searches) за вікно.
- Telemetry-розширення (дешеве, поверх наявного telemetry_events): події 'event_import', 'social_proof_impression' {kind}, 'search'. Жодного нового телеметрі-екрана — лише нові name-и через telemetry::log.
- Concierge-онбординг helper: seed-скрипт/адмін-дія для позначення організатора як seeded (0% fee вже досягається cost_type='free'/price=0 → NoopProvider; НЕ вводимо новий fee-tier).

**Поза scope:**
- Реальний платіжний провайдер, платні tiers, demand-маркетинг — за межами (вмикається після gate, окрема ініціатива).
- Повний автоматичний скрейпінг Telegram/Instagram із логіном/обходом rate-limit/anti-bot — заборонено; лише best-effort парсинг публічного тексту/OG.
- Recommendation-алгоритм переписування (наявний discover-ranking лишається; лише додаємо non-friend соц-доказ поля).
- Ticket tiers, quantity, Door-staff, gifting, guest-checkout — окремі TASK-и (TIERS/STAFF/…); тут НЕ реалізуються.
- Автоматичне блокування public-запуску кодом — gate_status лише інформативний сигнал для оператора, НЕ feature-flag, що глушить API.
- ML/дедуплікація імпортованих подій, cross-organizer merge — P2+, не тут.

**Модель даних:**
- Міграція 0033_coldstart.sql: ALTER TABLE events ADD COLUMN source TEXT, ADD COLUMN source_url TEXT, ADD COLUMN imported_by UUID REFERENCES users(id); частковий індекс events_import_idx ON events(imported_by) WHERE source='import'.
- Жодної нової таблиці для соц-доказу — going_city_count/nearby_popular/organizer_followers_count обчислюються запитами поверх наявних tickets/follows/user_prefs/cities.
- Жодної нової таблиці для телеметрії — використати наявну telemetry_events(user_id, name, props JSONB, created_at) з 0016; нові name: 'event_import','search','social_proof_impression'.
- Config/пороги launch-gate: додати до payment_settings або окремої config-таблиці рядки liquidity_min_events_per_week (int, deflt 15), liquidity_min_active_organizers (int, deflt 20), liquidity_min_search_success (numeric bps, deflt 7000=70%). Читаються admin liquidity endpoint-ом.
- EVENT_COLS розширити на source, source_url, imported_by (для serialization у EventDto).

**API:**
- POST /api/organizer/events/import — auth: is_organizer; body {source_url?: string, raw_text?: string} (принаймні одне); resp 201 {event_id, prefilled: EventDraftDto, unresolved_fields: string[]}; створює events row status='draft', source='import', imported_by=uid; best-effort парсинг OG/тексту; НЕ мінтить квитки.
- GET /api/discover — розширити DiscoverEvent: +going_city_count:i64, +nearby_popular:bool, +organizer_followers_count:i64 (обчислені відносно viewer.city/geo; анонім → going_city_count за подієвим містом).
- GET /api/events/:id/social — розширити EventSocial: +going_city_count:i64, +organizer_followers_count:i64, +nearby_popular:bool.
- POST /api/telemetry {name:'search', props:{city, has_query, result_count, zero_result}} — вже існує endpoint; лише новий name зі сторони discovery/пошуку (сервер логує 'search' у самому GET /api/events хендлері).
- GET /api/admin/liquidity?city=&days= — CurrentAdmin; resp {window_days, city, live_events_per_week:f64, active_organizers:i64, search_success_rate:f64, thresholds:{...}, gate_status:'PASS'|'FAIL', per_criterion:[{name, value, threshold, pass:bool}]}.
- GET /api/admin/telemetry — наявний; переконатися що 'search'/'event_import'/'social_proof_impression' зʼявляються в 7-денному зведенні (без змін коду, лише нові name).

**UI:**
- Web /organizer: кнопка «Створити з посилання/афіші» → модалка з полями URL / вставити текст → POST import → редірект у форму редагування чернетки з підсвіченими unresolved_fields; бейдж «Афіша (import)» на картці чернетки.
- EventCard (Web + Android): один рядок соц-доказу під датою·містом за fallback-пріоритетом friends_going → «N з <місто> йдуть» → «Популярне поблизу» → «Організатора стежать X»; показувати лише якщо значення значуще (>0 / true); клієнт шле social_proof_impression раз на показ (debounced).
- Деталі події: соц-блок доповнено рядком non-friend доказу коли друзів немає (щоб не було порожньо).
- Admin /liquidity: селектор міста + вікна (7/14/30д); три картки-критерії зі значенням/порогом/світлофором (зелений pass, червоний fail) + агрегований бейдж gate_status PASS/FAIL; підпис «Інформативний сигнал готовності до public-запуску».
- Admin events list: колонка/бейдж source='import' щоб оператор бачив засіяні події.

**Acceptance Criteria:**
- AC-1: POST /api/organizer/events/import з валідним source_url або raw_text створює подію status='draft', source='import', imported_by=caller, повертає 201 + prefilled поля; квитки НЕ мінтяться, подія НЕ показується в GET /api/events (лише published).
- AC-2: import із порожнім body (ні source_url, ні raw_text) → 422 Validation; неавторизований або non-organizer → 401/403.
- AC-3: best-effort парсинг заповнює якнайбільше з {title, description, starts_at, city, venue_address, poster_url}; поля, які не вдалося витягти, повертаються у unresolved_fields, а не вигадуються (жодних placeholder-значень).
- AC-4: GET /api/discover повертає going_city_count/nearby_popular/organizer_followers_count; для анонімного viewer going_city_count рахується за містом події, для авторизованого — за viewer.city; значення невідʼємні й консистентні з tickets/follows.
- AC-5: GET /api/events/:id/social повертає ті самі non-friend поля; жоден із цих запитів НЕ звертається до escrow.rs/payments (соц-нейтральність).
- AC-6: EventCard показує рівно ОДИН рядок соц-доказу за визначеним fallback-пріоритетом; коли всі значення нульові/false — рядок не рендериться (не показуємо «0 йдуть»).
- AC-7: GET /api/events (параметризований пошук) логує telemetry 'search' з zero_result=(result_count==0); search_success_rate у /liquidity = 1 - zero/total за вікно і збігається з підрахунком напряму з telemetry_events.
- AC-8: GET /api/admin/liquidity повертає три критерії з value/threshold/pass та gate_status='PASS' лише коли ВСІ три критерії pass; пороги читаються з config, не хардкод у хендлері.
- AC-9: гейт інформативний — жоден публічний endpoint не блокується значенням gate_status (немає прихованого feature-flag).

**Security-вектори (SEC):**
- SEC-1: import обмежений is_organizer + rate-limit (напр. ≤20 import/год/користувач) щоб запобігти масовому спаму чернеток; imported_by завжди = автентифікований uid, не з body.
- SEC-2: SSRF при парсингу source_url — блокувати приватні/loopback/metadata діапазони (169.254.0.0/16, 10/8, 127/8, ::1), тільки http(s), таймаут+ліміт розміру відповіді; жодних редіректів на internal.
- SEC-3: stored-XSS/ін'єкція з парсеного HTML — санітизувати/екранувати title/description перед збереженням; зберігати як plain text, не HTML.
- SEC-4: liquidity/telemetry endpoint-и лише CurrentAdmin; витік PII заборонено — /liquidity повертає агрегати, не списки користувачів.
- SEC-5: соц-доказ не має деанонімізувати — «N з міста йдуть» лише лічильник (going_city_count>=порог, напр. приховувати точні аватари для non-friends), organizer_followers_count лише число; жодних імен незнайомців без follow-звʼязку.
- SEC-6: telemetry 'search'/'social_proof_impression' props валідуються за розміром (≤64 name, обмежений JSON) — наявний log_event guard; клієнт не може інжектити довільні великі props.
- SEC-7: draft-подія від import не потрапляє в анонімну видачу (status='draft' фільтрується) — перевірити що import не обходить published-гейт.

**Test Matrix:**
- POS: import(source_url з OG-метаданими) → 201, поля title/starts_at заповнені, status=draft, source=import.
- POS: import(raw_text київської афіші) → 201, title/city/starts_at витягнуто, решта в unresolved_fields.
- POS: discover як анонім → going_city_count за містом події; як авторизований у Києві → going_city_count за Києвом.
- POS: /admin/liquidity з даними що перевищують усі пороги → gate_status=PASS, три pass:true.
- POS: search із result_count>0 логує 'search' zero_result=false; success_rate зростає.
- NEG: import без body → 422; non-organizer → 403.
- NEG: /admin/liquidity одним критерієм нижче порога → gate_status=FAIL, саме той критерій pass:false.
- NEG: EventCard з усіма нульовими соц-полями → рядок соц-доказу відсутній.
- NEG: discover не повертає draft/imported-неопубліковані події анонімам.
- [SEC] import(source_url=http://169.254.169.254/…) → відхилено (SSRF-guard), подія не створена.
- [SEC] import(source_url=http://127.0.0.1:5432) → відхилено.
- [SEC] import(raw_text з <script>) → title/description збережені як екранований plain text, без активного HTML.
- [SEC] import 21-й раз за годину → 429 rate-limit.
- [SEC] GET /api/admin/liquidity без ADMIN → 401/403.
- [SEC] social-proof для non-friend не повертає імен/аватарів незнайомців, лише агреговані лічильники.
- [SEC] telemetry props >дозволеного розміру → відхилено наявним guard.

---

