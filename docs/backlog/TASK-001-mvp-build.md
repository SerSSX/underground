# TASK-001 — Build Underground MVP→v1

**Status:** DECOMPOSED
**Created:** 2026-06-12  **Updated:** 2026-06-12 (auth: додано email+пароль поряд із Google; делівері — частинами)

## Raw Request
> (Verbatim — do NOT edit after intake)
>
> «Давай зробимо ауторизацію через гугл аут. Тема буде завжди темна. Весь інший функціонал такий як на дизайні.
> Продумай ще настройки і адмін панель для керування платформою. Платіжку якусь типу ту що ми в сігнал опсі додавали.
> Продумай реалізацію та задачі і зроби план реалізації.»
>
> Уточнення: «Диви давай виплати авторам за квитки будуть після проведення події, що би не було так що гроші зібрані
> а ніхто не провів цей івент. Має бути продумана система яка сама буде регулюватися комюніті і адміном що би все було
> чесно і беспечно. Гроші мають локатись а потім після виплачуватись організаторам.»
>
> Доктрина: «Юзай ці рули для розробки і тільки їх» (→ `docs/rules.md` v0.5.0).
> Рішення розвилок: гейти — «строго інфра зараз»; раніше написаний M0-код — «откатити + переробити»;
> адмінка — окремий веб; платіж — Lemon Squeezy (MoR); монетизація — комісійний маркетплейс з ескроу.

## Refined Description
**Scope:** мобільна платформа (афіша + квитки + соцмережа) — Android-клієнт (Compose, dark-only), Rust/Axum/SQLx API,
окремий Next.js web-admin. Auth = Google OAuth. Платіж = Lemon Squeezy (MoR), комісійний маркетплейс з **ескроу**
(гроші утримуються до завершення події; реліз/refund регулюються спільнотою+ADMIN). Функціонал — за дизайном Figma.

**Non-goals (поза MVP→v1):** справжні авто-спліт-виплати на кожен продаж (LS не вміє → ledger+сетлмент); реалтайм-чат (WS);
повноцінний trust-score організатора; нативний iOS; phone/OTP-вхід (замінено Google).

**Impacted UCs:** UC-001…UC-011 (нові; визначені в `docs/spec.md`).
**Impacted BR/WF:** BR-001…BR-015; WF-Auth, WF-Escrow.
**Dependencies:** зовнішні — Google OAuth, Lemon Squeezy (sandbox-ключі).
**Risks / Open Questions:**
- LS ≠ marketplace split → `PAID_OUT` як окремий сетлмент (узгоджено).
- LS one-time для квитків: динамічний checkout vs варіант-на-подію — уточнити в Speck UC-004.
- Web-only пункти правил (Playwright §7.3, Angular §8.4) застосовні до `admin/`; Android UI — Compose/instrumented.

## Estimation
**Level:** HIGH
**Justification:** велика площа поведінки (11 UC), три кодові поверхні, гроші/ескроу, нова інфра. Неможливо верифікувати
атомарно → MUST decompose (§12.5/§12.6). Implementation листя дозволена лише коли воно `LOW`.

## Decomposition

Виконання — **інфра-first** (рішення «строго інфра зараз»): спершу епік-INFRA (leaves A–F, усі `LOW`, READY),
лише потім фіч-епіки. Кожен фіч-епік (UC) сам `HIGH/MEDIUM` і **DECOMPOSED** у `LOW` BE/FE/INT-листя у власному файлі
ПЕРЕД його реалізацією (§12.6.1). Поки епік не декомпозовано — його реалізація заборонена.

### Епік INFRA — `TASK-001-A…F` (leaves, виконати першими)

| Sub-task ID | Title | Est. | Status | Depends On |
|---|---|---|---|---|
| TASK-001-A | docker-compose dev (Postgres+API), порти через `.env` (§8.1) | LOW | DONE | — |
| TASK-001-B | `scripts/ci.sh` fail-fast стадії + `.dockerignore` (§6.5,§8.6) | LOW | DONE | A |
| TASK-001-C | Rust gates: `clippy -D warnings`, `fmt --check`, `cargo llvm-cov` 100% (§7.1) | LOW | DONE | A |
| TASK-001-D | API bootstrap: config/db/error/міграції + `/health` (контейнеризовано) | LOW | DONE | A,C |
| TASK-001-E | Android skeleton: Retrofit+Nav+dark theme + test harness (+ ранній UC-002/003 зріз на запит) | LOW | DONE | — |
| TASK-001-F | Admin Next.js skeleton + Playwright device-matrix harness (§7.3) | LOW | READY | A |

### Епіки FEATURE — per-UC (декомпозувати у власних файлах перед реалізацією)

| Epic ID | UC | Title | Est. | Status | Depends On |
|---|---|---|---|---|---|
| TASK-002 | UC-001 | Auth: email/пароль (Google — follow-up) | MEDIUM | DONE | INFRA |
| TASK-003 | UC-002 | Афіша: список/сітка, пошук, фільтри | MEDIUM | DECOMPOSED-pending | TASK-002 |
| TASK-004 | UC-003 | Деталі події + «цікаво» | LOW | DECOMPOSED-pending | TASK-003 |
| TASK-005 | UC-006 | Створення івенту (+ Мої події, FAB) | MEDIUM | DONE | TASK-002 |
| TASK-006 | UC-004/007 | Квитки (free reserve) + e-квиток QR + Мої квитки | MEDIUM | DONE | TASK-005 |
| TASK-008 | UC-004/010 | Платний потік (Lemon Squeezy) + ескроу HELD→… | HIGH | DECOMPOSED-pending | TASK-006 |
| TASK-007 | UC-007 | QR e-квиток + сканер/валідація | MEDIUM | DECOMPOSED-pending | TASK-006 |
| TASK-008 | UC-010 | Ескроу-рушій (tick, clearing, dispute, refund) | HIGH | DECOMPOSED-pending | TASK-006 |
| TASK-009 | UC-005 | Передача/продаж/повернення квитка | MEDIUM | DECOMPOSED-pending | TASK-008 |
| TASK-010 | UC-009 | Скарги + рейтинги | LOW | DECOMPOSED-pending | TASK-008 |
| TASK-011 | UC-008 | Соц: профіль/підписки/пости/чати | HIGH | DECOMPOSED-pending | TASK-002 |
| TASK-012 | UC-011 | Web-admin: модерація + ескроу/виплати | HIGH | DECOMPOSED-pending | TASK-008,F |

## Execution Order
1. **INFRA:** A → (C, E паралельно) → B, D, F.
2. **FEATURE:** TASK-002 → 003 → {004, 005} → 006 → {007, 008} → {009, 010, 012} ; TASK-011 паралельно після 002.
3. Хардненг (security-review §9.1, повний CI §8.6, demo-тест §7.6) — фінальний гейт перед «done» (§11).

## Speck Reference
Кожне листя/епік отримує `docs/specks/SDD-###-*.md` перед реалізацією (§3.1). Парент-ACs верифікуються end-to-end,
коли всі листя `DONE` (§12.7).
