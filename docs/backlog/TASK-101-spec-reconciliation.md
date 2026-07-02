# TASK-101 — Реконсиляція spec.md: product-first + auth-доктрина + minor-units/timezone

**Status:** DONE
**Created:** 2026-06-29  **Updated:** 2026-07-01

> **Виконано (2026-07-01, founder-approved auth-доктрина §10):** усі листя 101-A…G внесено:
> `CLAUDE.md` (auth: Google + passwordless email-код; платіж: product-first/provider-neutral);
> `docs/spec.md` — §2.1 product-first доктрина; UC-001 + WF-Auth під email-код; нові UC-012 (gift-claim),
> UC-013 (browse-before-auth), UC-014 (door-staff), UC-015 (analytics); BR-003/006/012/017 оновлено,
> нові BR-018…BR-022 (unclaimed, minor-units, event_timezone, product-first payout-gate, claim-token);
> WF-GiftClaim + WF-DoorScan; §3/§4 (зовнішні сервіси, актори). Розблоковує TASK-102/104/110/113/114/115/118.
**Initiative:** INIT-ROADMAP-SPEC (Phase 0)  **Speck:** docs/specks/SDD-101-spec-reconciliation.md

## Raw Request
> «У репо два взаємовиключні роадмапи (money-first vs product-first) — це порушення §0. Зафіксуй у spec.md єдину доктрину: product-first, payment last. Вирішити auth-питання (Google-only vs frictionless claim) і оновити CLAUDE.md. Додати каркас нових UC під фічі, від яких залежать усі інші задачі: gift-claim без акаунта, browse-before-auth, делеговане door-scan, аналітика. Плюс дешеві незворотні контракти — minor-units і event_timezone — щоб не робити breaking-міграції на живому продукті. Це НЕ код фіч — це узгодження spec.»

## Refined Description
**Scope:** Оновити `docs/spec.md` як єдине джерело істини (rules §0/§2.2) під v2 БЕЗ реалізації фіч-коду:
1. **Product-first доктрина** — зафіксувати «product-first, payment last»: перший реліз (Simple Launch) на free/RSVP + `NoopProvider`; реальні гроші/ledger/reconciliation/KYC — Фаза 4. Пороги готовності релізу (share→claim ≥30%, guest→signup ≥40%, K ≥0.5). Escrow — provider-neutral state machine; активний провайдер на старті = Noop/Test.
2. **Auth-доктрина** — розв'язати конфлікт «Google-only vs frictionless claim»: Google Sign-In для повних акаунтів + passwordless email-код **ЯК ВИНЯТОК** у claim/guest-контексті + identity-merge за email. Оновити `CLAUDE.md` рядок «тільки Google OAuth» → «Google OAuth + passwordless email-код у claim/guest як задокументований виняток». Уточнити UC-001 і WF-Auth. Потрібен явний апрув засновника (§10 — зміна auth-доктрини).
3. **UC-каркас нових фіч** (бізнес-рівень: Summary/Description/FR/Related — детальні §2.3-контракти в кожному Speck пізніше):
   - **UC-012 Gift-claim** (ticket `unclaimed`/`pending_claim`, claim-token, забір без акаунта);
   - **UC-013 Browse-before-auth** (анонім бачить афішу/деталі; auth лише на дію; anon-intent merge);
   - **UC-014 Door-staff delegation** (event-scoped `event_staff`, event-scoped `validate`, вердикти);
   - **UC-015 Analytics/Tracking** (telemetry funnel, K-factor атрибуція).
4. **BR/WF-каркас** — нові BR (unclaimed-квиток, комісія-НЕ-з-подарованих, minor-units канон, event_timezone, anti-passback, product-first payout-gate) + WF-GiftClaim, WF-DoorScan; оновити WF-Auth під email-код.
5. **Незворотні контракти** — зафіксувати в spec як істину: гроші = minor-units i64 на межі API/ledger (єдина конверсія в `money.rs`); кожна подія має `event_timezone` (IANA), час показується в поясі події; i18n-каркас + guardrail проти substring-рендеру часу.

**Non-goals:** Будь-який фіч-код (міграції, endpoints, UI) — це роблять залежні задачі (TASK-CONTRACTS-1, INIT-TICKET-UNCLAIMED, INIT-GIFTLINK, INIT-DOORSTAFF, INIT-ANALYTICS тощо). Тут — ЛИШЕ текст `docs/spec.md`, `docs/rules.md` (guardrail-рядки), `CLAUDE.md`. Жодного дотику `api/src/*`, `android/**`, `web/**`, `admin/**`. Масовий переклад 306 кирилиць-літералів — окремий борг (не тут).

**Impacted UCs:** UC-001 (оновлення), UC-004/UC-005 (комісія-нейтральність gift/free — уточнення), UC-007 (event-scoped scan — уточнення), UC-010 (product-first payout-gate — уточнення); **нові UC-012..UC-015** (new UC required — вводяться цією задачею).

**Impacted BR/WF:** оновити BR-003 (комісія НЕ з gift/free), BR-007/BR-016/BR-017 (email-код виняток), BR-012 (event-scoped + anti-passback); **нові** BR-018 (unclaimed-квиток), BR-019 (minor-units канон), BR-020 (event_timezone), BR-021 (product-first: payout лише коли активний PSP live, до того Noop/Test), BR-022 (single-use short-TTL claim-token окремим секретом). WF-Auth (оновити), **нові** WF-GiftClaim, WF-DoorScan.

**Dependencies:** — (Phase-0 корінь; блокує TASK-CONTRACTS-1, INIT-TICKET-UNCLAIMED, INIT-BROWSE-AUTH, INIT-GIFTLINK, INIT-DOORSTAFF, INIT-ANALYTICS — усі посилаються на «spec-виняток внесено ПЕРЕД кодом»). Зовнішня: **апрув засновника** на auth-доктрину (§10) і product-first M1→Фаза 4.

**Risks / Open Questions:**
- (R1) Зміна auth-доктрини в CLAUDE.md = зміна governing-summary → потрібен явний founder-approval ПЕРЕД мержем (§0/§10); без нього — блок.
- (R2) Ризик drift: якщо UC-каркас надто детальний, суперечитиме майбутнім Speck. Мітигація: тримати bізнес-рівень, делегувати §2.3-контракти в per-UC Speck.
- (R3) Нумерація UC/BR — переконатися, що UC-012..015 і BR-018..022 не колідують з наявними. Перевірено проти spec.md (max UC-011, BR-017).
- (Q1) Чи `pending_claim` і `unclaimed` — один стан чи два? Рішення для spec: один канонічний стан `unclaimed` (термін `pending_claim` = синонім у прозі), фіналізується в INIT-TICKET-UNCLAIMED Speck.
- (Q2) Видалити старий money-first роадмап-файл чи позначити superseded? Рішення: spec.md явно оголошує product-first єдиним; конфліктні аналітичні файли позначаються «superseded by INIT-ROADMAP-SPEC» (не видаляються — історія аудиту).

## Estimation
**Level:** HIGH → **декомпозовано** (§12.5/§12.6). Хоч це docs-only, обсяг перевищує LOW-листок: чотири незалежні concern-и (product-first доктрина; auth-доктрина + governing CLAUDE.md-зміна з founder-approval; UC/BR/WF-каркас на 4 нові фічі; minor-units/timezone контракти) з різними рецензентами й різним рівнем ризику (auth-зміна — §10 approval-gated). Кожен concern — самодостатній review-юніт. Тому HIGH, розбито на LOW-листя ПЕРЕД будь-яким текстом spec.

## Sub-tasks (DECOMPOSED)
| Sub-task ID | Title | Estimation | Status | Depends On |
|---|---|---|---|---|
| TASK-101-A | spec.md §product-first доктрина (payment-last, Simple Launch, release-gates, escrow provider-neutral, BR-021) | LOW | READY | — |
| TASK-101-B | spec.md UC-001 + WF-Auth оновлення під email-код виняток; CLAUDE.md auth-рядок (founder-approval gate, §10) | LOW | READY | 101-A |
| TASK-101-C | spec.md нові UC-012 (gift-claim) + UC-013 (browse-before-auth) + BR-018/BR-022 + WF-GiftClaim | LOW | READY | 101-B |
| TASK-101-D | spec.md новий UC-014 (door-staff) — event-scoped scan/вердикти/anti-passback + BR-012 update + WF-DoorScan | LOW | READY | 101-A |
| TASK-101-E | spec.md новий UC-015 (analytics/tracking funnel + K-factor) — бізнес-рівень | LOW | READY | 101-A |
| TASK-101-F | spec.md контракти: minor-units i64 (BR-019) + event_timezone (BR-020); rules.md guardrail-рядки (substring-time заборона, stringResource) | LOW | READY | 101-A |
| TASK-101-G | Реконсиляція: позначити money-first аналіз-файли «superseded»; крос-лінк UC↔INIT↔TASK; фінальна spec-consistency перевірка | LOW | READY | 101-A..F |

## Execution Order
101-A → 101-B (auth-approval паралельно можна ініціювати) → {101-C, 101-D, 101-E, 101-F паралельно} → 101-G (закриваючий consistency-pass). 101-B і 101-C послідовні (C посилається на email-код доктрину з B). Жоден листок не запускає фіч-код — усі виводять лише текст docs.

## Speck Reference
docs/specks/SDD-101-spec-reconciliation.md
