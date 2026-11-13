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

## L1 notes (web i18n, 2026-11-13)

Built: the contract below, the language picker, browser-language detection, Intl dates and plurals, category
labels and API messages through the catalogs. Translations themselves are L2: `web/locales/{es,pt,fr,de,it,pl,tr,ja}.json`
are `{}`, so those languages show English until L2 fills them (never Ukrainian).

### Contract (call sites)

- `tr(lang, "Афіша", "Discover")` — static text.
- `tf(lang, "Усі події · {city}", "All events · {city}", { city })` — text with `{name}` placeholders.
- `tp(lang, n, { one: "{n} місце", few: "{n} місця", many: "{n} місць" }, { one: "{n} place", other: "{n} places" }, vars?)`
  — plural; `{n}` is the count, other vars as in `tf`. The Ukrainian object always has one/few/many, the English one/other.
- Lookup: `uk` → the Ukrainian text; `en` → the English text; any other language → `locales/<lang>.json[<exact Ukrainian
  source>]`, else the English text. A missing, empty or wrong-shaped entry falls back to English.
- Plural selection is `Intl.PluralRules(lang)`; a missing plural entry falls back to the English forms chosen by
  English rules (ja n=1 → "1 place", not "1 places").
- Arguments must be literals, so `scripts/i18n-extract.mjs` can read them. Computed text is only allowed on a line
  marked `i18n-dynamic` whose keys come from a data module the extractor reads: `lib/categories.mjs` (category
  labels) and `lib/server-messages.mjs` (API texts).
- Code: `lib/i18n-core.mjs` (pure: tr/tf/tp, languages, catalogs; types in `.d.mts`), `lib/i18n.tsx` (React provider,
  lazy catalog loading, re-exports), `lib/dates.mjs` (Intl dates), `lib/server-messages.mjs`, `lib/categories.mjs`.

### Catalog shape (`web/locales/<lang>.json`, what L2 translators fill)

One JSON object; keys are the exact Ukrainian source strings from `locales/_source.json` (do not edit keys).

```json
{
  "Афіша": "Descubrir",
  "Усі події · {city}": "Todos los eventos · {city}",
  "{n} місце": { "one": "{n} lugar", "other": "{n} lugares" }
}
```

- Static and template entries: a non-empty string. Every `{placeholder}` of the source must appear, none added.
- Plural entries (the source value is an object): an object keyed by the TARGET language's CLDR categories. Required
  (integer counts): es, de, it, tr, pt, fr: `one`, `other`; pl: `one`, `few`, `many` (`other` allowed, fractions only);
  ja: `other` only. A category the language does not have is an error (ja `one`). A form may leave out `{n}`
  ("un lugar"), never another placeholder.
- `locales/_source.json` is the translator's input: the same shape, with the English text as the value (plural:
  the English `one`/`other` forms). 536 keys, 14 of them plural, from 499 calls + the two data modules.

### Tools and gates

- `node scripts/i18n-extract.mjs` rewrites `_source.json`; `--check` exits 1 when it is stale. Errors: non-literal
  call, `tr` with placeholders, uk/en placeholders differ, a key used as both plural and text. Warnings (not errors):
  one Ukrainian source with two English texts — 6 today ("Увійти" Log in / Sign in, "Поскаржитися", "Повідомлення"
  Messages / Message, "Бронювання закрите", "Сканер", "Створити акаунт"); other languages get ONE translation for each.
- `node scripts/i18n-check.mjs` exits 1 on any problem, missing keys included (the L2 bar); `--allow-missing` reports
  them only.
- `lib/i18n.test.mjs` (in CI's `node --test lib/*.test.mjs`): `_source.json` up to date and no extraction errors,
  catalog shapes/placeholders (missing allowed), the checker's own negative cases, fallback order, never-Ukrainian
  over the whole source, `tf`, plurals uk/en/pl/ja, browser-language pick, dates ja/tr/uk in a zone, server texts.
  Mutation-checked: 8 mutants of the guards (fallback to uk, English re-select, uk nominative, zone dropped,
  pickLang, English-code mapping, two checker rules) — all killed. **Adding a `tr/tf/tp` call means re-running
  the extractor, or CI goes red.**

### Behaviour

- Picker: endonyms (Українська … 日本語) in a native select — profile menu, signed-out top bar (compact globe + code,
  visible on mobile too), /account. Signed-in users on a phone use the avatar menu or /account.
- First visit: first supported language in `navigator.languages` (uk → uk, ru skipped, nothing supported → en), stored in
  `localStorage.ug_lang`; a stored valid choice wins. `<html lang>` follows after hydration. The last pick wins when
  two catalogs load out of order.
- Server prefs locale: the API keeps uk/en only (`api/src/prefs.rs` maps others to uk), so the web sends `uk` for
  Ukrainian and `en` for everything else — via `syncUserLocale` and now also `putPrefs` (it used to omit the locale,
  and the API reset it to uk whenever interests were saved).
- Dates: `Intl.DateTimeFormat` in the event's zone (en uses en-GB, day before month); Ukrainian long date keeps the
  nominative weekday (CLDR gives "пʼятницю, 20 листопада"). Months print as CLDR has them ("лист", was "лис").
- API errors: `check()` keeps the Ukrainian source text (English codes such as "email taken", "not found", and
  fetch's "Failed to fetch" are mapped to Ukrainian texts first); `errorMessage()` shows it in the active language.
  Unknown server texts are shown as sent. Discover section titles go through the same table.

### Not localised yet (known gaps)

- Server-rendered metadata stays Ukrainian: `app/layout.tsx`, `app/page.tsx`, every `app/**/layout.tsx` title, the
  `c/[category]`, `city/[city]` and `events/[eventId]` metadata (titles, descriptions, Open Graph) and JSON-LD; the
  first paint is Ukrainian until hydration. Per-language URLs for SEO = later.
- Legal pages `/privacy`, `/terms`, `/abuse`, `/refunds` — Ukrainian only (need a real translation, not L2 strings).
- Notifications, emails and push texts — the API's uk/en only; other languages get English.
- Server texts not in `lib/server-messages.mjs` (any new API message, admin-only messages, most English validation
  codes) show as the server wrote them. User content (titles, names, "Гість") is never translated.
- Default city "Київ" in the event form and onboarding (G2 replaces it with city search).
- Numbers are plain (`1000`, not `1 000`/`1,000`); times are always 24-hour.
- The home hero is two fragments ("Куди піти " + "сьогодні?") — translators must keep the split.
- Not checked in a real browser in this unit (no server run): hydration switch, picker on a phone, catalog chunks.

## G1 notes (api/, 2026-11-13)

Built: `GET /api/geo/search`, `GET /api/geo/reverse` (Photon proxy, cache, rate limit); cities learned on demand;
events accept a chosen point and country; the event zone follows its coordinates. Migration `0049_geo.sql`.

Contracts (for G2):
- `GET /api/geo/search?q=&kind=city|address&lang=xx&lat=&lng=` → `200 [GeoPlace]` (≤ 8). `q` 2..120 characters after
  trimming and collapsing spaces; `kind` default `address` (= everything), `city` = place:city/town/village/hamlet;
  `lang` = two lowercase letters, sent upstream as `en`/`de`/`fr` or else `default` (local names: Photon rejects
  every other language, `it` included); `lat`+`lng` optional bias, both or neither, rounded to 0.1° before use.
- `GET /api/geo/reverse?lat=&lng=&lang=xx` → `200 [GeoPlace]` (0..1); the point is rounded to 0.001° (~110 m).
- `GeoPlace = {label, name, street, housenumber, city, state, country, country_code, lat, lng, kind}`; strings may be
  null; `kind` = `city|address|place`; `city` is the locality (a city result's own name); `country_code` upper case.
- Errors: 422 `{"error"}` bad input; 429 more than `GEO_LIMIT_PER_MIN` (60) a minute per client IP (rightmost
  X-Forwarded-For); 503 `{"error":"пошук адрес тимчасово недоступний"}` on upstream timeout (5 s), non-2xx, redirect,
  body over 256 KB, bad JSON, or the global cap `GEO_UPSTREAM_LIMIT_PER_MIN` (300 upstream calls/min).
- Events: `POST /api/events` and `PUT /api/events/:id` accept optional `lat`, `lng` (both or neither, ±90/±180, not
  0,0), `country` (≤ 80 chars), `country_code` (2 letters). Event JSON gains `country_code`. A point wins over the
  city-name lookup. Without an explicit `timezone` the zone comes from the place; send no `timezone` (or "") to get it.
  On PUT, sending any of `city`/`lat`/`lng`/`country`/`country_code` re-settles country, code and coordinates
  (NULL when nothing is known: the old city's coordinates no longer linger); the zone changes only when a new one is
  known or `timezone` is sent.
- `GET /api/cities` rows gain `country_code` and `timezone`.

Decisions taken here (orchestrator to confirm):
- Time zone crate: `tzf-rs` 2.3.0 is MIT, but its bundled data crate `tzf-dist` is `MIT AND ODbL-1.0` (OSM-derived
  boundaries), outside the licence allowlist, and so is every polygon-accurate alternative. Fallback built: `tz.rs`,
  ISO country → zone, nearest reference city for the 22 multi-zone countries; with no country, the nearest gazetteer
  city with a zone within 300 km; else `Europe/Kyiv` as before. Seeded cities carry exact zones (migration).
  Near a zone border inside a multi-zone country (US, CA, MX, BR, RU, AU, ID, KZ…) the guess can be off; the
  organiser's explicit zone always wins. If the owner accepts ODbL (Photon's data is ODbL already), `tzf-rs` replaces it.
- New dependency `reqwest` 0.12 (MIT OR Apache-2.0), `default-features = false`, `rustls-tls`: its tree adds no
  licence outside the allowlist except `webpki-roots` (CDLA-Permissive-2.0), already in the tree through sqlx.
- Gazetteer learning: a city is added only when a `city` result with the same name is in a cached geocoder answer
  (≤ 7 days), within 50 km of the event's point if one is sent, otherwise unambiguous (one country, or narrowed by
  `country_code`). The client's text and point alone never create a row. Names stay unique case-insensitively
  across countries (0015's index): discovery joins `cities` by name, so a second "Springfield" is not added; its
  events still keep their own point and zone.
- Currency default prefers `country_code` (`currency::for_country_code`): geocoder country names come in many languages.
- Production needs egress from the API container to `photon.komoot.io:443`; attribution "© OpenStreetMap
  contributors" belongs in the web UI (G2).
