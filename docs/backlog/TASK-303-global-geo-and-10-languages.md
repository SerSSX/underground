# TASK-303 — Global: any country/city, address search, geolocation, 10 languages

Status: IN PROGRESS (2026-11-13) · Lane: PROJECT · brief mode: quick (2 gaps asked) · parent: TASK-300

## Request (owner, 2026-11-13, verbatim)

> Так, давай доробимо таку штуку. Коротше, я задумав це як глобальний додаток, тобто там має бути доступні, не
> знаю, всі країни, всі міста, умовно кажучи, щоб ти міг вибрати і там пошукати, створити навіть в будь-якому типу
> місці. От. Треба це реалізувати. Можливо, додати можливість визначення геопозиції, і якщо у нас немає списку
> населеного пункту, то туди його додати. Також треба, щоб була ну, така, знаєш, зручна можливість вибирати адреса,
> тобто вводити дані, треба якісь бібліотеки підключити, щоб ми мали доступ до там, всіх адресів, не знаю. І можна
> було ще м, натиснути на кнопку відкрити в Google картах, де воно там знаходиться, ця власне подія чи щось таке.
> Тобто не будемо локалізуватися тільки на Києві, ну там не тільки на Україні. І власне локалізації теж давай під
> популярних, не знаю, там штук 10 мов хай буде. На старті буде нормально. Тобто по дизайну виглядає непогано, але
> я ще все не подивився. Ну, от, продивлюсь весь функціонал, дам остаточний фідбек, що будемо ще по функціоналу
> робити. Зараз локалізацію треба проробити, щоб вона була глобальна. Це важливо.

## Owner answers (2026-11-13)

- Languages: «Європа + світ» — uk, en, es, pt, fr, de, it, pl, tr, ja (no Russian).
- Address/city search: «OpenStreetMap, безкоштовно» — Photon (OSM) through our API with a cache; no paid keys.

## Units (each: build → distinct review → CI → DEV)

- G1 (api/): geo search + reverse proxy to Photon with cache and rate limit; cities added on demand with the time
  zone from coordinates; events store lat/lng from the chosen address; the event zone follows its coordinates.
- L1 (web/): i18n for 10 languages: catalogs, language picker, browser-language detection, Intl dates/plurals,
  categories, API messages shown in the visitor's language.
- G2 (web/): city search anywhere, "near me" (geolocation), address autocomplete in the event form, "Open in
  Google Maps" from coordinates. After L1 (its texts go through the catalogs).
- L2: translations of the full catalog into the 8 remaining languages; spot review.
