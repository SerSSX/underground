# TASK-199 — План Фаз 2-4 (батьківські планові задачі ініціатив)

| Поле | Значення |
|---|---|
| **ID** | TASK-199 |
| **Тип** | Parent / Planning (Фази 2-4) |
| **Status** | **INTAKE (planned)** — кожна дитина переходить у повний lifecycle при вході у свою фазу |
| **Parent** | TASK-100 |
| **Estimation** | **HIGH → буде декомпозовано пофазно** (§12.5/§12.6) |
| **Джерела** | `docs/analysis/2026-06-29-audit-consolidated-roadmap.md` §4/§8, `docs/analysis/2026-06-29-payments-worldwide.md` |
| **Дата** | 2026-07-01 |

---

## 1. Raw Request

> Спланувати Фази 2-4 роадмапу як майбутні батьківські задачі. Кожна ініціатива (`INIT-*`) = майбутня MEDIUM/HIGH задача, що буде декомпозована у LOW-листя при плануванні відповідної фази. Окремо зафіксувати: реальний PSP (**Stripe Connect + Payoneer**, юр-особа US Delaware C-corp через Stripe Atlas) вмикається у **Фазі 3-4** за seam **TASK-104** — ОСТАННІМ, після PMF (product-first, payment last).

---

## 2. Refined Description

### 2.1. Scope

TASK-199 — планова батьківська задача. Тримає **майбутні** ініціативи Фаз 2-4 у стані `INTAKE (planned)` до моменту, коли попередня фаза `DONE` (§11) і фаза відкривається. На вході у фазу кожна ініціатива:

1. піднімається до повного lifecycle `INTAKE → REFINED → ESTIMATED → READY` (§12);
2. отримує спарований `docs/specks/SDD-###-*.md` (`AC-*` + **`SEC-*`** + Test Matrix, §3);
3. будь-яка зміна поведінки — спершу в `docs/spec.md` (§2.2), з посиланням «new UC required (див. TASK-101)»;
4. проходить gating (§12.5): MEDIUM → flag+confirm; HIGH → MUST decompose у LOW-листя.

### 2.2. Non-goals

- Жоден код Фаз 2-4 не пишеться, доки Фаза 1 (Simple Launch) не `DONE` і фаза не відкрита.
- Не будуємо зараз: повний risk-console, KYB-для-всіх, важку A/B-платформу, FX/RTL, map-view, seating, multi-day, recurring, Wallet-паси (роадмап §6 — заморожено до density в Києві).

### 2.3. Dependencies / Risks

- Кожна дитина залежить від конкретних Фаза-0/1 задач (стовпець «Enable-після» нижче).
- Реальний PSP — окремий трек: юр-особа (Atlas) + sandbox-інтеграція паралельні, але **enable в prod — ОСТАННІМ**; ризики AUP/payout-reach/reserves — у §5.

---

## 3. Фаза 2 — Простота відкриття, довіра, supply-активація

> Мета: Афіша — двигун відкриття; escrow — видима обіцянка; організатор активований <3хв; retention-петлі; готувати ліквідність до public.

| Планова TASK | Ініціатива | Прогноз | Gate | Enable-після | Межі декомпозиції (§12.6) |
|---|---|---|---|---|---|
| TASK-199-A | INIT-DISCOVERY — time-aware персональна Афіша | MEDIUM | flag+confirm | Фаза 1 done | API discover-секції / Android вертикаль / fallback-ланцюг |
| TASK-199-B | INIT-TRUST-ESCROW — escrow-as-trust + dispute/refund + early-payout | MEDIUM | flag+confirm | TASK-111 | trust-бейджі UI / dispute-типи API / early-payout апсел / air-alert авто-резолвер |
| TASK-199-C | INIT-QUICKCREATE — Luma-mode create + KYC JIT + post-publish share | MEDIUM | flag+confirm | TASK-102 | schema-дефолти / API publish / Android quick-form / share-екран |
| TASK-199-D | INIT-LIFECYCLE-TRIGGERS — ~5 named-тригерів + throttle-gate | MEDIUM | flag+confirm | TASK-111, TASK-117 | outbox-тригери / throttle-gate / digest |
| TASK-199-E | INIT-COLDSTART — supply-seeding playbook + non-friend соц-доказ + liquidity gate | MEDIUM | flag+confirm | — (операційно паралельно) | import-seam / single-player value / liquidity-gate метрика |
| TASK-199-F | INIT-SEO — SSR home + JSON-LD + гео/категорійні лендінги | MEDIUM | flag+confirm | — (паралельний трек) | зняти `'use client'` / JSON-LD / `/c` `/city` роути |

## 4. Фаза 3 — Платежі, ticket-lifecycle, зростання

> Мета: реальні гроші за provider-agnostic seam з простим checkout; exchange/transfer/refund у коректний домен; ріст organizer-каналом + базова соцмережність.

| Планова TASK | Ініціатива | Прогноз | Gate | Enable-після | Межі декомпозиції (§12.6) |
|---|---|---|---|---|---|
| TASK-199-G | INIT-CHECKOUT — простий-за-замовчуванням checkout + all-in + native wallet | **HIGH → decompose** | MUST decompose | Реальний PSP (seam TASK-104) | progressive disclosure UI / all-in ціна / wallet express / idempotent post-payment |
| TASK-199-H | INIT-TICKET-LIFECYCLE — іменні квитки + transfer + per-ticket escrow/refund | **HIGH → decompose** | MUST decompose | TASK-110, TASK-102 | schema per-ticket FSM / transfer-seam атомарність / refund-політика конфіг / double-spend guard |
| TASK-199-I | INIT-ORGANIZER-CHANNEL — branded URL + embed-widget + UTM/referral | **HIGH → decompose** | MUST decompose | TASK-117 | branded URL / iframe widget / UTM-атрибуція / promoter split на escrow-seam |
| TASK-199-J | INIT-RSVP-SOCIAL — RSVP-driver + приватність + базова соцмережність + anti-spam | MEDIUM | flag+confirm | TASK-111 | RSVP-toggle / privacy-фільтр / feed / rate-limit |
| TASK-199-K | INIT-LOCALE-ASO — UA-first локалізація + локальний PSP day-1 + ASO | MEDIUM | flag+confirm | TASK-102, TASK-101 | 100% UA UI / IBAN-payout без SWIFT / UA-PSP за seam / ASO-артефакт |
| TASK-199-L | INIT-EXPERIMENTS — feature-flag + exposure seam для A/B | MEDIUM | flag+confirm | TASK-117 | hash-bucketing / `experiment_exposed` event / перші експерименти |
| TASK-199-M | INIT-DESIGN-TOKENS — token-контракт + non-color redundancy + системні стани | MEDIUM | flag+confirm | розблоковує M9 | `design-tokens.css` lint / non-color redundancy / states / a11y-sweep |

## 5. Реальний PSP (Stripe Connect + Payoneer) — вмикається ОСТАННІМ

> **Незмінний інваріант product-first, payment last.** Реальні гроші вмикаються у **Фазі 3-4** за seam **TASK-104** (`escrow.rs` / `payouts.rs` / `orders.rs` / `webhooks.rs`). До PMF усі грошові шляхи — через `NoopProvider`/`TestProvider`.

| Планова TASK | Компонент | Прогноз | Gate | Мапінг на seam TASK-104 |
|---|---|---|---|---|
| TASK-199-N | **PSP-PRIMARY** — Stripe Connect Express (separate charges & transfers) | **HIGH → decompose** | MUST decompose | `HELD` = charge на platform balance; `PAID_OUT` = Transfer API; `REFUNDED` = Refund/Reversal; `DISPUTED` = webhook `charge.dispute.created` |
| TASK-199-O | **PSP-PAYOUT-FALLBACK** — Payoneer Mass Payouts (UA + ~150 країн поза Stripe) | **HIGH → decompose** | MUST decompose | `PAID_OUT`-гілка маршрутизується у Payoneer payout API за країною організатора (UA не є Stripe payout-destination) |
| TASK-199-P | **PSP-ENTITY** — US Delaware C-corp через Stripe Atlas (юр-трек засновника) | MEDIUM (operational) | flag+confirm | Розблоковує prod-enable; sandbox-розробка (TASK-199-N/O) — паралельно, БЕЗ entity |

**Ключові рішення (з payments-worldwide.md):**

- Custody завжди на provider balance (Stripe platform balance) — Underground **не** money transmitter; seam лише викликає Transfer/Refund.
- `CLEARING`/`CLEARED` лишаються **чисто внутрішніми** станами ledger — жодного Stripe-виклику; Stripe-рух лише на межах `HELD` (charge) і `PAID_OUT`/`REFUNDED`.
- Провайдер вводиться за `payment_settings.provider='stripe'` гілкою **паралельно** наявному шляху — НЕ ламати existing (§10, no breaking API).
- Sandbox-повний escrow-флоу (charge→hold→transfer→payout→refund→dispute) будується без юр-особи; тест-картки `4242…` (успіх) / `4000…0259` (dispute).
- Ризики під контроль: AUP account-freeze (явно описати Connect-модель Stripe), payout-reach gap (Payoneer з дня 1), reserves/cash-flow, hold-cap (US-platform до 2 років для далеких подій), provider-lock-in у `webhooks.rs` (міграція за флагом).

## 6. Фаза 4 — Real-money back-office, вторинний ринок, compliance

> Мета: грошова коректність промислового рівня; face-value resale після ліквідності.

| Планова TASK | Ініціатива | Прогноз | Gate | Enable-після | Межі декомпозиції (§12.6) |
|---|---|---|---|---|---|
| TASK-199-Q | INIT-COMPLIANCE-GUARDS — ліміти делегованих грошей + GDPR-erasure телеметрії + report-петля | MEDIUM | flag+confirm | реальні гроші (TASK-199-N/O) | staff-refund cap+co-sign / transfer undo / telemetry в erasure / report auto-freeze |
| TASK-199-R | INIT-RESALE — face-value P2P-resale + резервування полів зараз | **HIGH → decompose** | MUST decompose | TASK-199-H + ліквідність | `resale_state` schema (зарезервувати зараз) / list-unlist-buy / стеля=номінал / escrow-викуп |

> Ledger double-entry / reconciliation / financial-KPIs / dispute-drawer / KYC/KYB queue / anti-fraud risk-console / bulk payouts — сюди ж (роадмап §5 «Відкладено у Фазу 4»), кожне окремим листком при декомпозиції.

## 7. Gating (§12.5) — зведення

- **MEDIUM-діти** (199-A/B/C/D/E/F/J/K/L/M/P/Q): flag+confirm на вході у фазу.
- **HIGH-діти** (199-G/H/I/N/O/R): MUST decompose у LOW-листя ПЕРЕД кодом; parent-init → `DECOMPOSED`.
- Жодна дитина не стартує, доки її «Enable-після» задача не `DONE` і фаза не відкрита.
- Реальний PSP (199-N/O) — **останній грошовий enable**; product-first інваріант TASK-100 §5.

## 8. Definition of Done

TASK-199 → `DONE` коли всі листя Фаз 2-4 термінальні за **`docs/rules.md` §11** (docs, infra, tests з `[SEC]`, Playwright device-matrix, security-audit без нових high/critical §9.1).
