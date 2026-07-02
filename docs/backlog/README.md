# Backlog — Underground v2

> Індекс усіх задач збірки **v2**. Керівна доктрина — `docs/rules.md` v0.5.0 (STRICT).
> Кожна задача має повний lifecycle (§12): `INTAKE → REFINED → ESTIMATED → READY` (або `DECOMPOSED`),
> спарований speck `docs/specks/SDD-###-*.md` (§3, з `AC-*` + **`SEC-*`** + Test Matrix),
> і не суперечить `docs/spec.md` (business truth, §2).
> Стратегія релізу: **product-first, payment last** — перший реліз (Simple Launch) на free/RSVP з `NoopProvider`;
> реальний PSP (Stripe Connect + Payoneer) вмикається ОСТАННІМ (Фази 3-4) за seam `TASK-104`.

## Легенда статусів (§12)

`INTAKE` → `REFINED` → `ESTIMATED` → `READY` → (в роботі) → `DONE`. Батьківські HIGH-задачі → `DECOMPOSED`.
Gating (§12.5): **LOW** — напряму · **MEDIUM** — flag+confirm · **HIGH/IMPOSSIBLE** — MUST decompose.

## Дерево планування

- **TASK-100** — батьківська задача всієї v2 (`DECOMPOSED`). Координує Фази 0-4.
  - Фаза 0-1 → окремі TASK/SDD нижче (TASK-101..118).
  - Фази 2-4 → **TASK-199** (планові батьківські задачі ініціатив).

## Таблиця v2-задач

| TASK | Назва | Фаза | Estimation | Gate (§12.5) | Status | Speck |
|---|---|---|---|---|---|---|
| **TASK-100** | Underground v2 Build (parent / release-train) | 0-4 | **HIGH → decomposed** | MUST decompose | `DECOMPOSED` | SDD-100 |
| **TASK-101** | Реконсиляція `spec.md`: product-first + auth-доктрина + minor-units/timezone | 0 | MEDIUM | flag+confirm | `READY` | SDD-101 |
| **TASK-102** | Незворотні контракти: minor-units гроші, `event_timezone`, i18n-каркас | 0 | MEDIUM | flag+confirm | `READY` | SDD-102 |
| **TASK-103** | CI/CD pipeline + container-first тест-харнес | 0 | MEDIUM | flag+confirm | `READY` | SDD-103 |
| **TASK-104** | Payment-provider seam: `trait` + Noop/Test провайдери + ledger | 0 | **HIGH → decomposed** | MUST decompose | `DECOMPOSED` | SDD-104 |
| **TASK-110** | Стан квитка `unclaimed` + подарунок на незареєстрований email | 1 | MEDIUM | flag+confirm | `READY` | SDD-110 |
| **TASK-111** | Канали доставки: FCM push + транзакційний email | 1 | MEDIUM | flag+confirm | `READY` | SDD-111 |
| **TASK-112** | Universal links + OG-превʼю для всіх share-обʼєктів | 1 | MEDIUM | flag+confirm | `READY` | SDD-112 |
| **TASK-113** | Anti-abuse контур навколо guest/gift-claim | 1 | MEDIUM | flag+confirm | `READY` | SDD-113 |
| **TASK-114** | Наскрізний frictionless gift/claim віральний контур | 1 | **HIGH → decomposed** | MUST decompose | `DECOMPOSED` | SDD-114 |
| **TASK-115** | Browse-before-auth + passwordless email-код | 1 | MEDIUM | flag+confirm | `READY` | SDD-115 |
| **TASK-116** | Не-блокуючий онбординг + contextual permission priming | 1 | **LOW** | proceed напряму | `READY` | SDD-116 |
| **TASK-117** | Tracking plan + воронка покупки + K-factor атрибуція | 1 | MEDIUM | flag+confirm | `READY` | SDD-117 |
| **TASK-118** | Делегований door-scan + безпечний QR + вердикти | 1 | MEDIUM | flag+confirm | `READY` | SDD-118 |
| **TASK-199** | План Фаз 2-4: батьківські задачі ініціатив + реальний PSP (last) | 2-4 | **HIGH → planned** | MUST decompose при вході у фазу | `INTAKE` (planned) | SDD-199 (per-init at scheduling) |

## Порядок виконання (з TASK-100 §5)

1. **Фаза 0:** `TASK-101 → TASK-102 → {TASK-103 ∥ TASK-104}`.
2. **Фаза 1:** прекурсори `TASK-110 ∥ 111 ∥ 112 ∥ 115 ∥ 117`, потім `TASK-113`, `TASK-116`; **`TASK-114` — ОСТАННІМ** (петля цілісно); `TASK-118` — паралельно.
3. **Фази 2-4:** через `TASK-199`, декомпозиція при вході у фазу.

## Release-gate Simple Launch (Фаза 1)

share→claim ≥ 30% · guest→signup ≥ 40% · K-factor ≥ 0.5 (роадмап §9.2, інструментовано TASK-117).

## Історичні задачі (v1)

Задачі v1 (TASK-001..016, M9, FEATURES) — у цьому ж каталозі; лишаються як історія попередньої збірки.
