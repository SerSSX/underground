import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { LANGS, CATALOG_LANGS, tr, tf, tp, pickLang, setCatalog, pluralCategory, serverLocale, templateParts } from "./i18n-core.mjs";
import { dateBadge, shortWhen, longDate, timeOf, isoDateLabel, zoneLabel } from "./dates.mjs";
import { serverText, canonicalServerText } from "./server-messages.mjs";
import { CATEGORY_LABELS } from "./categories.mjs";
import { extract, serialize, SOURCE_PATH } from "../scripts/i18n-extract.mjs";
import { checkCatalogs } from "../scripts/i18n-check.mjs";

const PLACES_UK = { one: "{n} місце", few: "{n} місця", many: "{n} місць" };
const PLACES_EN = { one: "{n} place", other: "{n} places" };

test("uk and en come from the call site", () => {
  assert.equal(tr("uk", "Афіша", "Discover"), "Афіша");
  assert.equal(tr("en", "Афіша", "Discover"), "Discover");
});

test("other languages: catalog first, then English — never Ukrainian", () => {
  setCatalog("es", { "Афіша": "Descubrir", "Порожнє": "", "Об'єкт": { one: "x" } });
  assert.equal(tr("es", "Афіша", "Discover"), "Descubrir");
  assert.equal(tr("es", "Квитки", "Tickets"), "Tickets", "missing key → English");
  assert.equal(tr("es", "Порожнє", "Empty"), "Empty", "empty translation → English");
  assert.equal(tr("es", "Об'єкт", "Object"), "Object", "wrong shape → English");
  assert.equal(tr("es", "constructor", "ctor"), "ctor", "no prototype keys");
  assert.equal(tr("fr", "Афіша", "Discover"), "Discover", "catalog not loaded → English");
});

test("with every catalog empty, no source string reaches a non-Ukrainian reader in Ukrainian", () => {
  const source = JSON.parse(readFileSync(SOURCE_PATH, "utf8"));
  for (const lang of CATALOG_LANGS) setCatalog(lang, {});
  for (const lang of LANGS.filter((l) => l !== "uk")) {
    for (const [uk, en] of Object.entries(source)) {
      if (typeof en === "string") assert.equal(tr(lang, uk, en), en, `${lang}: ${uk}`);
      else assert.equal(tp(lang, 2, { one: uk, few: uk, many: uk }, en), tp("en", 2, { one: uk }, en), `${lang}: ${uk}`);
    }
  }
});

test("tf interpolates {name} in every language and leaves an unknown placeholder visible", () => {
  assert.equal(tf("uk", "Усі події · {city}", "All events · {city}", { city: "Київ" }), "Усі події · Київ");
  assert.equal(tf("en", "Усі події · {city}", "All events · {city}", { city: "Kyiv" }), "All events · Kyiv");
  setCatalog("de", { "Усі події · {city}": "Alle Events · {city}" });
  assert.equal(tf("de", "Усі події · {city}", "All events · {city}", { city: "Köln" }), "Alle Events · Köln");
  assert.equal(tf("it", "Усі події · {city}", "All events · {city}", { city: "Roma" }), "All events · Roma", "fallback still interpolates");
  assert.equal(tf("en", "Місце {i} з {total}", "Place {i} of {total}", { i: 2, total: 5 }), "Place 2 of 5");
  assert.equal(tf("en", "a {x} b", "a {x} b", {}), "a {x} b");
  assert.equal(tf("en", "{n} МБ", "{n} MB", { n: 0 }), "0 MB", "zero is a value, not missing");
});

test("templateParts: text with slots, so a translation may move a link but never add markup", () => {
  assert.deepEqual(templateParts("Read the {terms} and {privacy}."), ["Read the ", { slot: "terms" }, " and ", { slot: "privacy" }, "."]);
  assert.deepEqual(templateParts("{terms}と{privacy}に同意したことになります。"), [{ slot: "terms" }, "と", { slot: "privacy" }, "に同意したことになります。"]);
  assert.deepEqual(templateParts("<b>x</b>"), ["<b>x</b>"], "markup stays text");
  assert.deepEqual(templateParts(""), []);
});

test("tp: Ukrainian one/few/many", () => {
  const cases = { 1: "1 місце", 2: "2 місця", 4: "4 місця", 5: "5 місць", 11: "11 місць", 12: "12 місць", 21: "21 місце", 22: "22 місця", 25: "25 місць", 111: "111 місць", 0: "0 місць" };
  for (const [n, want] of Object.entries(cases)) assert.equal(tp("uk", Number(n), PLACES_UK, PLACES_EN), want);
});

test("tp: English one/other, extra vars", () => {
  assert.equal(tp("en", 1, PLACES_UK, PLACES_EN), "1 place");
  assert.equal(tp("en", 2, PLACES_UK, PLACES_EN), "2 places");
  assert.equal(tp("en", 0, PLACES_UK, PLACES_EN), "0 places");
  const uk = { one: "Заброньовано {booked} з {n} місця", few: "Заброньовано {booked} з {n} місць", many: "Заброньовано {booked} з {n} місць" };
  assert.equal(tp("en", 3, uk, { one: "{booked} of {n} place booked", other: "{booked} of {n} places booked" }, { booked: 1 }), "1 of 3 places booked");
});

test("tp: Polish one/few/many from the catalog", () => {
  setCatalog("pl", { "{n} місце": { one: "{n} miejsce", few: "{n} miejsca", many: "{n} miejsc", other: "{n} miejsca" } });
  const cases = { 1: "1 miejsce", 2: "2 miejsca", 4: "4 miejsca", 5: "5 miejsc", 12: "12 miejsc", 22: "22 miejsca", 25: "25 miejsc", 0: "0 miejsc" };
  for (const [n, want] of Object.entries(cases)) assert.equal(tp("pl", Number(n), PLACES_UK, PLACES_EN), want, `pl ${n}`);
  assert.equal(pluralCategory("pl", 22), "few");
  assert.equal(pluralCategory("pl", 12), "many");
});

test("tp: Japanese has one form; a missing entry falls back to English with English rules", () => {
  setCatalog("ja", { "{n} місце": { other: "{n}席" } });
  assert.equal(tp("ja", 1, PLACES_UK, PLACES_EN), "1席");
  assert.equal(tp("ja", 7, PLACES_UK, PLACES_EN), "7席");
  setCatalog("ja", {});
  assert.equal(tp("ja", 1, PLACES_UK, PLACES_EN), "1 place", "ja selects 'other', English must re-select 'one'");
  assert.equal(tp("ja", 3, PLACES_UK, PLACES_EN), "3 places");
});

test("first visit: the browser's first supported language, else English", () => {
  assert.equal(pickLang(["uk-UA", "en"]), "uk");
  assert.equal(pickLang(["ru-RU", "uk"]), "uk");
  assert.equal(pickLang(["ru"]), "en", "Russian is not offered");
  assert.equal(pickLang(["pt-BR"]), "pt");
  assert.equal(pickLang(["ja-JP"]), "ja");
  assert.equal(pickLang(["TR"]), "tr");
  assert.equal(pickLang(["zh-CN", "ko"]), "en");
  assert.equal(pickLang([]), "en");
  assert.equal(pickLang(undefined), "en");
});

test("server prefs get uk or en only (the API maps anything else to uk)", () => {
  assert.equal(serverLocale("uk"), "uk");
  for (const l of LANGS.filter((x) => x !== "uk")) assert.equal(serverLocale(l), "en");
});

const FRI_NOV_20 = "2026-11-20T10:00:00Z"; // 19:00 in Tokyo, 13:00 in Istanbul, 12:00 in Kyiv

test("dates: Japanese in the event's zone", () => {
  assert.equal(timeOf(FRI_NOV_20, "Asia/Tokyo"), "19:00");
  assert.deepEqual(dateBadge(FRI_NOV_20, "ja", "Asia/Tokyo"), { day: "20", mon: "11月" });
  const s = shortWhen(FRI_NOV_20, "ja", "Asia/Tokyo");
  assert.match(s, /^11月20日\(金\) · 19:00$/, s);
  assert.equal(longDate(FRI_NOV_20, "ja", "Asia/Tokyo"), "11月20日金曜日");
  // 22:30 UTC is already Saturday the 21st in Tokyo: the zone decides the date, not the visitor.
  assert.match(shortWhen("2026-11-20T22:30:00Z", "ja", "Asia/Tokyo"), /^11月21日\(土\) · 07:30$/);
});

test("dates: Turkish puts the day first and the weekday last", () => {
  assert.equal(longDate(FRI_NOV_20, "tr", "Europe/Istanbul"), "20 Kasım Cuma");
  assert.deepEqual(dateBadge(FRI_NOV_20, "tr", "Europe/Istanbul"), { day: "20", mon: "Kas" });
  assert.match(shortWhen(FRI_NOV_20, "tr", "Europe/Istanbul"), /^20 Kas Cum · 13:00$/);
});

test("dates: Ukrainian heading uses the nominative weekday", () => {
  assert.match(longDate(FRI_NOV_20, "uk", "Europe/Kyiv"), /^п.ятниця, 20 листопада$/);
  assert.match(shortWhen(FRI_NOV_20, "uk", "Europe/Kyiv"), /^пт, 20 лист · 12:00$/);
  assert.deepEqual(dateBadge(FRI_NOV_20, "uk", "Europe/Kyiv"), { day: "20", mon: "лист" });
});

test("dates: unknown zone falls back instead of throwing; bad input gives empty", () => {
  assert.ok(shortWhen(FRI_NOV_20, "en", "Not/AZone").includes("Nov"));
  assert.equal(shortWhen("nonsense", "en", "Europe/Kyiv"), "");
  assert.equal(dateBadge(null, "en"), null);
  assert.equal(isoDateLabel("2026-06-15", "en"), "15 Jun");
  assert.equal(isoDateLabel("2026-06-15T19:05:00Z", "ja"), "6月15日 · 19:05");
});

test("zone labels: the generic name in the reader's language, else the IANA city", () => {
  assert.equal(zoneLabel("Asia/Tokyo", "ja"), "日本標準時");
  assert.equal(zoneLabel("Europe/Lisbon", "en"), "Western European Time");
  assert.match(zoneLabel("Europe/Kyiv", "uk"), /східноєвропейськ/);
  assert.equal(zoneLabel("Not/A_Zone", "en"), "A Zone", "unknown zone → its city part, no throw");
  assert.equal(zoneLabel("Etc/GMT-3", "de"), "GMT-3", "an offset-only Intl name → the IANA part, as written");
});

test("server texts: known ones translate, unknown ones stay as sent", () => {
  assert.equal(serverText("uk", "квитки закінчились"), "квитки закінчились");
  assert.equal(serverText("en", "квитки закінчились"), "no places left");
  setCatalog("es", { "квитки закінчились": "no quedan plazas" });
  assert.equal(serverText("es", "квитки закінчились"), "no quedan plazas");
  assert.equal(serverText("fr", "квитки закінчились"), "no places left", "no catalog entry → English");
  assert.equal(serverText("en", "якась нова помилка"), "якась нова помилка");
  assert.equal(serverText("en", "Помилка 502"), "Error 502");
  assert.equal(serverText("en", "на одну людину — не більше 5 місць (у вас уже 3)"), "at most 5 places per person (you already have 3)");
  assert.equal(serverText("en", "email taken"), "This email is already registered");
  assert.equal(serverText("uk", "Failed to fetch"), "Немає зʼєднання з сервером");
  assert.equal(canonicalServerText("unauthorized"), null, "status fallback decides");
  assert.equal(canonicalServerText(""), null);
});

test("category keys are lower-case and every label pair is filled", () => {
  for (const [k, [uk, en]] of Object.entries(CATEGORY_LABELS)) {
    assert.equal(k, k.toLowerCase());
    assert.ok(uk && en, k);
  }
});

test("locales/_source.json is up to date with the code, and every call is extractable", async () => {
  const { source, errors } = await extract();
  assert.deepEqual(errors, []);
  assert.equal(readFileSync(SOURCE_PATH, "utf8"), serialize(source), "run: node scripts/i18n-extract.mjs");
});

test("catalogs: shapes and placeholders match the source, and no key is missing in any language", () => {
  const { errors } = checkCatalogs();
  assert.deepEqual(errors, []);
});

test("the catalog check catches what it promises to", () => {
  const source = { "Усі події · {city}": "All events · {city}", "{n} місце": { one: "{n} place", other: "{n} places" }, "Так": "Yes" };
  const ok = { "Усі події · {city}": "x {city}", "{n} місце": { one: "{n} a", few: "{n} b", many: "{n} c", other: "{n} d" }, "Так": "Tak" };
  assert.deepEqual(checkCatalogs({ source, catalogs: { pl: ok } }).errors.filter((e) => e.startsWith("pl")), []);
  const bad = (cat, lang = "pl") => checkCatalogs({ source, catalogs: { [lang]: cat }, allowMissing: true }).errors.filter((e) => e.startsWith(lang));
  assert.equal(bad({ "Усі події · {city}": "x {miasto}" }).length, 1, "renamed placeholder");
  assert.equal(bad({ "Усі події · {city}": "x" }).length, 1, "dropped placeholder");
  assert.equal(bad({ "{n} місце": { one: "{n} a", many: "{n} c" } }).length, 1, "Polish needs few");
  assert.equal(bad({ "{n} місце": "{n} miejsc" }).length, 1, "plural given as text");
  assert.equal(bad({ "Так": { one: "x" } }).length, 1, "text given as plural");
  assert.equal(bad({ "Так": "" }).length, 1, "empty text");
  assert.equal(bad({ "{n} місце": { one: "{n}席", other: "{n}席" } }, "ja").length, 1, "Japanese has no 'one'");
  assert.equal(bad({ "{n} місце": { one: "un lugar", other: "{n} lugares" } }, "es").length, 0, "'one' may leave out {n}");
  assert.equal(bad({ "{n} місце": { one: "{n} {x}", other: "{n} lugares" } }, "es").length, 1, "invented placeholder");
  assert.ok(checkCatalogs({ source, catalogs: { pl: {} } }).errors.some((e) => e.includes("missing")), "missing fails without allowMissing");
});

test("TASK-305: rate-limit wording, the sign-up variant, the early-scan pattern and unknown English", async () => {
  const { serverText, isUnknownEnglish } = await import("./server-messages.mjs");
  assert.equal(serverText("uk", "забагато спроб, спробуйте пізніше"), "Забагато спроб. Спробуйте трохи пізніше.");
  assert.equal(serverText("uk", "забагато спроб, спробуйте пізніше", "signup"), "Забагато спроб з вашої мережі. Спробуйте за годину.");
  assert.equal(serverText("en", "забагато спроб, спробуйте пізніше", "signup"), "Too many attempts from your network. Please try again in an hour.");
  assert.notEqual(serverText("de", "забагато спроб, спробуйте пізніше", "signup"), "Забагато спроб з вашої мережі. Спробуйте за годину.", "translated");
  assert.equal(serverText("uk", "Ще рано: подія починається 14.11.2026 о 19:00"), "Ще рано: подія починається 14.11.2026 о 19:00");
  assert.equal(serverText("en", "Ще рано: подія починається 01.07.2030 о 20:00"), "Too early: the event starts on 01.07.2030 at 20:00", "the API's own format");
  assert.equal(serverText("en", "Ще рано: подія починається завтра."), "Too early: the event starts завтра", "any other form passes through");
  assert.equal(serverText("uk", "title: не більше 140 символів"), "Текст задовгий: не більше 140 символів", "no English field name");
  const iso = serverText("en", "Ще рано: подія починається 2026-11-14T17:00:00Z");
  assert.match(iso, /^Too early: the event starts \S/);
  assert.doesNotMatch(iso, /2026-11-14T/, "an ISO date is shown as a date");
  assert.equal(serverText("uk", "sent"), "Пропозицію надіслано — квиток перейде, коли одержувач прийме");
  assert.equal(serverText("uk", "reason is required (max 200 chars)"), "Опишіть причину скарги (до 200 символів)");
  assert.equal(isUnknownEnglish("lat and lng go together"), true);
  assert.equal(isUnknownEnglish("invalid email"), false, "mapped code");
  assert.equal(isUnknownEnglish("квитки закінчились"), false, "Ukrainian");
  assert.equal(isUnknownEnglish("Помилка 502"), false);
  assert.equal(isUnknownEnglish("Error 502"), true);
});

test("TASK-305: common categories are labelled, and an unknown one is shown capitalised, not as a raw key", async () => {
  const { CATEGORY_LABELS, categoryFallback } = await import("./categories.mjs");
  for (const k of ["music", "art", "comedy", "kids", "tech", "food", "film", "literature", "sport"]) assert.ok(CATEGORY_LABELS[k], k);
  assert.equal(categoryFallback("live coding"), "Live coding");
  assert.equal(categoryFallback("  ёлка  "), "Ёлка");
  assert.equal(categoryFallback(""), "");
});
