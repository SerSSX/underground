# SDD-200 — Секційна time-aware Афіша

**Task:** TASK-200  **Spec:** BR-024, UC-Discovery  **Status:** DRAFT → IN PROGRESS

## Acceptance Criteria
- **AC-1** `GET /api/discover/sections` повертає `{ sections: [{ key, title, events: DiscoverEvent[] }] }`; кожна подія
  зберігає наявні discovery-поля (`reason`, `saved_by_me`, `saves_count`, `distance_km`).
- **AC-2** Секція `soon` містить лише `published` події з `starts_at ∈ (now, now+48h)`, відсортовані `starts_at ASC`.
- **AC-3** Секція `weekend` містить події найближчих Сб–Нд (`date_trunc('week',now)+5d .. +7d`), `starts_at ASC`.
- **AC-4** Секція `for_you` присутня ЛИШЕ коли viewer автентифікований і має сигнал (user_prefs АБО event_saves АБО follows);
  використовує наявний персональний ranking. Для анонімів/без-сигналу — відсутня.
- **AC-5** Секція `popular` присутня ЗАВЖДИ (cold-start fallback), сортування за momentum (saves+tickets+reviews) DESC.
- **AC-6** Порожні секції не повертаються; порядок: for_you → soon → weekend → popular.
- **AC-7** Наявний `GET /api/discover` (плоский) не змінюється (backward-compat, §10).

## Security ACs
- **SEC-1** Endpoint auth-optional; персоналізація лише для автентифікованого viewer (bind `$1 = viewer_id`), інакше — публічний trending.
- **SEC-2** Viewer НЕ бачить власні події у секціях (як у наявному discover: `organizer_id <> viewer`).
- **SEC-3** Лише `status='published'` події (жодних hidden/cancelled/draft) у будь-якій секції.
- **SEC-4** SQL — виключно bind-параметри (без інтерполяції viewer-вводу); секційні фільтри — статичні літерали.

## Test Matrix
| # | Вектор | Очікування |
|---|---|---|
| T1 [pos] | анонім GET sections | soon/weekend/popular присутні (за наявності даних), for_you відсутня |
| T2 [pos] | user із prefs+saves | for_you присутня і перша |
| T3 [pos] | подія за 12 год | у `soon`; подія через 10 днів — ні |
| T4 [pos] | подія цих вихідних | у `weekend` |
| T5 [neg] | немає майбутніх подій | soon/weekend порожні → не повертаються; popular може бути порожня → не повертається |
| T6 [SEC] | hidden/cancelled подія | у жодній секції |
| T7 [SEC] | власна подія організатора | не потрапляє у його секції |
| T8 [pos] | backward-compat | `GET /api/discover` (плоский) незмінний |
