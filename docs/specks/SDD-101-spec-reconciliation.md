# SDD-101 — Реконсиляція spec.md: product-first + auth-доктрина + minor-units/timezone

**Backlog:** TASK-101  **Initiative:** INIT-ROADMAP-SPEC (Phase 0)
**Impacted UCs:** UC-001 (update), UC-004/005/007/010 (clarify), **UC-012..UC-015 (new)**
**Impacted BR/WF:** BR-003/007/012/016/017 (update), **BR-018..BR-022 (new)**, WF-Auth (update), **WF-GiftClaim / WF-DoorScan (new)**

## Scope / Non-goals
**Scope:** Виключно правки текстових docs — `docs/spec.md` (business truth, rules §2.2), `docs/rules.md` (guardrail-рядки), `CLAUDE.md` (governing-summary auth-рядок, approval-gated §10). Зафіксувати product-first/payment-last доктрину, розв'язати auth-конфлікт (Google + passwordless email-код-виняток + identity-merge), ввести бізнес-рівневий каркас UC-012..UC-015 і BR-018..BR-022 + WF-GiftClaim/WF-DoorScan, зафіксувати незворотні контракти minor-units i64 та event_timezone. Це — coordination-артефакт: усі залежні feature-задачі посилаються на нього як на spec-передумову.

**Non-goals:** Жодного коду/міграцій/тестів у `api/src/*.rs`, `android/app/src/main/java/com/underground/app/ui/**`, `web/app/**`, `admin/app/**`. Жодних §2.3 per-UC технічних контрактів (endpoints/schematics/baselines) — вони фіналізуються в кожному feature-Speck (SDD-CONTRACTS-1, SDD-GIFTLINK, SDD-DOORSTAFF, SDD-ANALYTICS, SDD-BROWSE-AUTH-001 тощо) ПЕРЕД їх кодом. Масовий переклад наявних кирилиць-літералів — окремий борг. Видалення старих аналіз-файлів (лише позначка «superseded»).

## Acceptance Criteria
- **AC-1:** `docs/spec.md` містить розділ «Доктрина реалізації» з явним твердженням «product-first, payment last»: перший реліз = free/RSVP на `NoopProvider`; реальні гроші/ledger/reconciliation/KYC/generic-webhook/bulk-payout — Фаза 4; активний провайдер на старті = Noop/Test; escrow лишається provider-neutral state machine (не абстракція провайдера). Зафіксовано пороги готовності: share→claim ≥30%, guest→signup ≥40%, K-factor ≥0.5.
- **AC-2:** UC-001 і WF-Auth оновлено: Google Sign-In — основний метод для повних акаунтів; passwordless email-код — задокументований **виняток** лише у claim/guest-контексті; identity-merge unclaimed-квитків за email при першому Google-логіні. `CLAUDE.md` рядок «Auth — тільки Google OAuth» замінено на формулювання, що включає email-код-виняток; правка позначена як потребує-founder-approval (§10) і не мержиться без нього.
- **AC-3:** `docs/spec.md` містить нові бізнес-рівневі UC-012 (Gift-claim без акаунта), UC-013 (Browse-before-auth), UC-014 (Door-staff delegation + event-scoped scan), UC-015 (Analytics/Tracking funnel), кожен із Summary/Description/Functional Requirements/Related; жоден не містить §2.3-технічних контрактів (делеговані в per-UC Speck із поміткою «new UC required — see TASK-101»).
- **AC-4:** Нові/оновлені BR у spec: BR-018 (квиток може бути `unclaimed`; escrow утримується за дарувальником; auto-revert по TTL), BR-019 (гроші = minor-units i64 на API/ledger; єдина NUMERIC↔minor конверсія в `money.rs`; домен рахує в Decimal), BR-020 (кожна подія має `event_timezone` IANA; час показується в поясі події), BR-021 (product-first: payout можливий лише коли активний PSP live; до того — Noop/Test, реальні гроші не рухаються), BR-022 (claim-token — single-use, short-TTL, окремий секрет від QR/JWT). Оновлено BR-003 (комісія НЕ береться з gift/free/guest-квитків), BR-012 (validate event-scoped + серверний atomic anti-passback first-write-wins за `qr_token/ticket_id`).
- **AC-5:** `docs/spec.md` містить нові Mermaid WF-GiftClaim (gift-link → unclaimed → публічна landing → одна кнопка «Забрати» → Google/email-код → атомарний owner-bind, first claim wins → active; auto-revert по TTL) і WF-DoorScan (event-scoped validate → вердикти ok|already_used|wrong_event|refunded|returned|inactive → anti-passback); WF-Auth оновлено гілкою email-код-виняток.
- **AC-6:** `docs/rules.md` доповнено guardrail-рядками: (a) заборона наївного substring-рендеру часу (`iso.substring`) — час завжди через `java.time`/`Intl` у поясі події; (b) нові Android-рядки — лише через `stringResource` (не літерал); (c) гроші на wire/ledger — minor-units i64, конверсія лише в money-seam.
- **AC-7:** Консистентність: у spec немає залишкового money-first формулювання, що суперечить product-first; конфліктні аналіз-файли позначені «superseded by INIT-ROADMAP-SPEC»; нумерація UC-012..015 і BR-018..022 не колідує з наявними (max було UC-011/BR-017); кожен новий UC має «Related» BR/WF, що реально існують у doc після правки (жодного висячого посилання).
- **AC-8:** Кожна залежна feature-задача-згадка (gift-claim, browse-auth, door-staff, tiers, analytics, contracts) має у spec явну позначку, що поведінка вводиться тут і реалізується в іменованій наступній задачі (traceability UC↔INIT↔TASK), тож жоден агент не почне фіч-код без цього spec-стану.

## Security Acceptance Criteria
- **SEC-1 (doctrine-integrity / policy-leak):** Розділ доктрини НЕ фіксує реальних секретів, ключів, env-значень, PSP-credentials чи endpoint-секретів у відкритому тексті spec/CLAUDE.md; описує лише поведінку й контракти. Перевірка grep на `API_KEY|SECRET|ls_api_key|Bearer ` у diff → порожньо.
- **SEC-2 (auth-doctrine downgrade-guard):** Формулювання email-код-винятку в spec/CLAUDE.md явно обмежує його claim/guest-контекстом і НЕ послаблює Google-only для повних акаунтів; явно вимагає single-use short-TTL код, max-attempts/burn, і що `/start` не розкриває існування акаунта (однакова відповідь). Виняток позначено §10-approval-gated — не набирає чинності без founder-approval.
- **SEC-3 (anti-abuse contract присутній як умова):** Для gift-claim/browse-auth spec фіксує обов'язковий anti-abuse контур (single-use + short-TTL claim-token окремим секретом, rate-limit, device/email-cap, magic-code, auto-revert, «забрано з іншого пристрою») як БЛОКУЮЧУ умову готовності — фіча не вмикається без нього; перелічує SEC-вектори для майбутніх Speck: token brute, replay, double-claim, claim-after-sold-out.
- **SEC-4 (escrow/money invariant збережено в spec):** Реконсиляція НЕ вводить жодного money-flow поза єдиним seam; spec явно повторює BR-006 (усі зміни грошей через `escrow.rs`/PaymentProvider seam) і що соц/gift/analytics-шляхи грошей не рухають (комісія НЕ з gift/free — BR-003 update). Anti-passback (BR-012) сформульовано як SEC-виправлення (first-write-wins), не опція.
- **SEC-5 (IDOR/ownership formulation):** UC-014 (door-staff) у spec фіксує STAFF як **per-event relationship**, НЕ глобальну роль; scan/list доступ — лише owner або per-event staff з відповідним perm; жодне формулювання не дозволяє крос-event доступ. UC-012 (gift-claim) фіксує атомарний owner-bind — лише перший авторизований claim виграє (проти double-claim IDOR).

## Failure Modes / Error Mapping
Це docs-only задача; «failure modes» — режими некоректної реконсиляції, що ловляться в review, а не рантайм-помилки:
- **Висяче посилання** (UC «Related» вказує на неіснуючий BR/WF) → review-fail (AC-7), фікс тексту.
- **Колізія нумерації** (повторний UC/BR-номер) → review-fail (AC-7).
- **Залишковий money-first** (суперечливе формулювання лишилось) → review-fail (AC-1/AC-7).
- **Auth-виняток без approval-позначки** → блок мержу (SEC-2, §10); не набирає чинності.
- **Просочення секрету в текст** → SEC-1 grep-gate fail, revert рядка.
- **Over-specification** (§2.3-технічний контракт затягнуто в spec-UC, створюючи майбутній drift) → review-fail (AC-3), винести в per-UC Speck.
- **Контракт-неузгодженість** (spec каже Decimal замість minor-units, або NUMERIC на wire) → fail AC-4/AC-6, узгодити з SDD-CONTRACTS-1.

## Test Matrix
> UI-стадії (Base UI/UI) — ⬜ N/A: задача не має UI/коду. «Тести» тут = doc-review gates + grep/lint на docs + traceability-перевірка (rules §6 адаптовано до docs-only артефакту; фіч-тести живуть у залежних задачах).

| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|---|---|---|---|---|---|---|---|
| AC-1 product-first доктрина | ✅ (doc-review: доктрина присутня+несуперечлива) | ✅ (крос-doc: spec vs roadmap узгоджені) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-2 auth-доктрина + CLAUDE.md | ✅ (review: UC-001/WF-Auth/CLAUDE.md формулювання) | ✅ (§10 approval-позначка присутня) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-3 нові UC-012..015 | ✅ (review: 4 UC, бізнес-рівень, без §2.3) | ✅ (traceability UC↔INIT↔TASK) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-4 BR-018..022 + updates | ✅ (review: тексти BR присутні+коректні) | ✅ (крос-ref BR↔UC несуперечливі) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-5 WF-GiftClaim/DoorScan Mermaid | ✅ (Mermaid парситься; гілки повні) | ✅ (WF↔UC↔BR узгоджені) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-6 rules guardrails | ✅ (review: 3 guardrail-рядки присутні) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-7 консистентність/нумерація | ✅ (grep: нема дублів UC/BR; нема money-first) | ✅ (superseded-позначки на конфлікт-файлах) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| AC-8 traceability | ✅ (кожна фіча-згадка → named наступна TASK) | ✅ (жодна feature-задача не стартує без spec) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-1 no secret leak | ✅ (grep API_KEY/SECRET/Bearer у diff → 0) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-2 auth downgrade-guard | ✅ (review: email-код обмежено claim/guest + approval-gate) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-3 anti-abuse контур як умова | ✅ (review: контур+SEC-вектори перелічені) | ✅ (feature-flag OFF-без-контуру зафіксовано) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-4 escrow/money invariant | ✅ (review: BR-006 повторено; gift/free/social no-money) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| SEC-5 IDOR/ownership формулювання | ✅ (review: staff per-event; atomic owner-bind first-wins) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
