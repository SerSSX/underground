# TASK-200 — Секційна time-aware Афіша (INIT-DISCOVERY leaf-1)

**Status:** DONE (verified: backend 100% cov + web live + Android device)  **Parent:** TASK-199-A  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

> **Реалізовано + перевірено:** additive `GET /api/discover/sections` (`discover.rs`: `section_events`/`has_signals`/
> `discover_sections`, реюз personal-ranking через `personal_order()`; плоский `/api/discover` рефакторено на той самий seam,
> discover/geo-тести зелені = 0 регресій). Секції soon/weekend/for_you(за сигналом)/popular; порожні опускаються.
> **Тести:** новий `tests/discover_sections.rs` (7: anon-no-for_you, for_you-first, no-signal, soon+weekend membership,
> empty-omit, [SEC] hidden+own excluded, flat-compat) + `fmt`+`clippy -D`+**100% coverage** (4511/4511).
> **Web (verified live browser):** `discoverSections` + home рендерить секції (snapshot: «Для тебе»/«Незабаром»/weekend/popular),
> 0 console-помилок. **Android (verified device):** `DiscoverSectionDto`/`discoverSections`, `AfishaUiState.Sections`+`reduceSections`,
> `AfishaScreen` рендерить заголовки секцій + картки; на телефоні видно секцію «Для тебе» з картками. compile+unit ✓.

## Raw Request
> Афіша — двигун відкриття. Замість плоского «Для тебе» — time-aware СЕКЦІЇ: «Незабаром», «Цими вихідними»,
> «Для тебе» (персоналізовано, лише за наявності сигналів), «Популярне» (cold-start fallback, завжди). Перше LOW-листя
> INIT-DISCOVERY: секційний discover-API + рендер секцій у клієнтах. Не ламати наявний `/api/discover` (§10).

## Refined Description
**Scope:**
- **API (additive, non-breaking):** новий `GET /api/discover/sections` → `{ sections: [{ key, title, events[] }] }`, кожна секція
  переюзає наявний discover-ranking/`DiscoverEvent` seam (`api/src/discover.rs`). Наявний плоский `GET /api/discover` не зачіпається.
  - Секції: `soon` «Незабаром» (starts_at у наступні 48 год), `weekend` «Цими вихідними» (найближчі Сб–Нд),
    `for_you` «Для тебе» (наявний персональний ranking, ЛИШЕ якщо у viewer є сигнали: prefs/saves/follows),
    `popular` «Популярне» (momentum DESC, завжди — cold-start fallback).
  - Порядок: for_you (якщо є) → soon → weekend → popular. Порожні секції не повертаються (fallback-ланцюг).
- **Клієнти:** Android Афіша-таб «Для тебе» рендерить вертикаль секцій (заголовок + горизонтальний/вертикальний ряд карток);
  web home так само. Плоский fallback лишається, якщо секцій нема.

**Non-goals:** ML-ранжування, map-view, «бо ти зберіг X»-explainability за подіями, тижнева нотифікація-digest (TASK-199-D),
timezone-perfect межі вихідних (використовуємо server-UTC-межі як задокументоване спрощення; BR-020 стосується показу, не фільтра).

**Impacted UC/BR:** UC-Discovery (розширюється секційним варіантом); **new BR-024** (секційний discover — spec-first, нижче).
Не змінює наявний `/api/discover` (BR ранжування лишається).

## Estimation
**Level:** LOW  **Justification:** один additive endpoint поверх наявного ranking-seam + рендер секцій; без схеми/міграцій,
без грошей, без breaking-змін. 100% coverage на 4 гілки секцій + has-signals.

## Speck Reference
docs/specks/SDD-200-discovery-sections.md
