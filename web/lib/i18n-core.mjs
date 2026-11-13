// i18n core (TASK-303 L1): languages, catalogs, and the three lookup functions every page uses.
// Pure module (no React, no DOM), so server components, lib/ui.ts and node:test can all import it.
//
// The contract (docs/backlog/TASK-303 "L1 notes" has the full catalog shape):
//  - Call sites carry the Ukrainian source and the English text: tr(lang, "Афіша", "Discover").
//  - uk → the Ukrainian string, en → the English string.
//  - Any other language → CATALOG[lang][<exact Ukrainian source>] if present, else the English string.
//    A non-Ukrainian reader never gets Ukrainian from a call site.
//  - Catalogs live in web/locales/<lang>.json and are registered at runtime with setCatalog().

export const LANGS = /** @type {const} */ (["uk", "en", "es", "pt", "fr", "de", "it", "pl", "tr", "ja"]);
export const DEFAULT_LANG = "uk";
/** Languages whose text comes from a catalog (uk and en come from the call sites). */
export const CATALOG_LANGS = /** @type {const} */ (["es", "pt", "fr", "de", "it", "pl", "tr", "ja"]);

/** Each language in its own name, for the picker. */
export const ENDONYMS = {
  uk: "Українська", en: "English", es: "Español", pt: "Português", fr: "Français",
  de: "Deutsch", it: "Italiano", pl: "Polski", tr: "Türkçe", ja: "日本語",
};

/** BCP 47 tag used for Intl formatting. English uses en-GB so dates read "Fri 20 Nov" (day before month). */
export const INTL_LOCALES = {
  uk: "uk", en: "en-GB", es: "es", pt: "pt", fr: "fr", de: "de", it: "it", pl: "pl", tr: "tr", ja: "ja",
};

/** Types: lib/i18n-core.d.mts. */
export function isLang(x) {
  return typeof x === "string" && /** @type {readonly string[]} */ (LANGS).includes(x);
}

/** First visit: the first supported language in the browser's list (navigator.languages), else English.
 *  "uk-UA" → uk, "pt-BR" → pt, "ru" → skipped (not offered), nothing supported → en. */
export function pickLang(list) {
  for (const tag of list ?? []) {
    if (typeof tag !== "string") continue;
    const primary = tag.trim().toLowerCase().split(/[-_]/)[0];
    if (isLang(primary)) return primary;
  }
  return "en";
}

/** @type {Record<string, Record<string, unknown>>} */
const catalogs = {};

/** Register a loaded catalog (web/locales/<lang>.json). uk/en never need one. */
export function setCatalog(lang, catalog) {
  catalogs[lang] = catalog && typeof catalog === "object" ? catalog : {};
}
export function hasCatalog(lang) {
  return Object.prototype.hasOwnProperty.call(catalogs, lang);
}

/** The locale stored in the user's server prefs. The API keeps notification texts in uk and en only
 *  (api/src/prefs.rs maps anything else to uk), so every other language asks for English. */
export function serverLocale(lang) {
  return lang === "uk" ? "uk" : "en";
}

let current = DEFAULT_LANG;
/** The active language for code that has no `lang` at hand (errorMessage in lib/api.ts). Set by LangProvider. */
export function setCurrentLang(lang) {
  if (isLang(lang)) current = lang;
}
export function currentLang() {
  return current;
}

function entry(lang, key) {
  const c = catalogs[lang];
  return c && Object.prototype.hasOwnProperty.call(c, key) ? c[key] : undefined;
}

/** Static pair. */
export function tr(lang, uk, en) {
  if (lang === "uk") return uk;
  if (lang === "en") return en;
  const v = entry(lang, uk);
  return typeof v === "string" && v !== "" ? v : en;
}

const PLACEHOLDER = /\{([A-Za-z_][A-Za-z0-9_]*)\}/g;

/** Replace {name} with vars.name; an unknown placeholder stays visible (loud, not silent). */
export function interpolate(template, vars) {
  if (!vars) return template;
  return template.replace(PLACEHOLDER, (m, k) => (Object.prototype.hasOwnProperty.call(vars, k) && vars[k] != null ? String(vars[k]) : m));
}

/** The placeholder names in a template, sorted and unique: "{a} of {b}" → ["a", "b"]. */
export function placeholders(template) {
  const out = new Set();
  for (const m of String(template).matchAll(PLACEHOLDER)) out.add(m[1]);
  return [...out].sort();
}

/** Template pair with {name} placeholders: tf(lang, "Усі події · {city}", "All events · {city}", { city }). */
export function tf(lang, uk, en, vars) {
  return interpolate(tr(lang, uk, en), vars);
}

/** @type {Map<string, Intl.PluralRules>} */
const rules = new Map();
/** CLDR plural category of n in a language ("one", "few", "many", "other", ...). */
export function pluralCategory(lang, n) {
  let r = rules.get(lang);
  if (!r) {
    r = new Intl.PluralRules(INTL_LOCALES[lang] ?? lang);
    rules.set(lang, r);
  }
  return r.select(n);
}

function pickForm(lang, n, forms) {
  if (!forms || typeof forms !== "object") return undefined;
  const cat = pluralCategory(lang, n);
  const f = forms[cat] ?? forms.other ?? forms.many;
  return typeof f === "string" && f !== "" ? f : undefined;
}

/** Plural with {n} (and any other vars):
 *  tp(lang, 5, { one: "{n} місце", few: "{n} місця", many: "{n} місць" }, { one: "{n} place", other: "{n} places" }).
 *  Catalog key = the Ukrainian "one" form; the catalog value is an object of the target language's categories. */
export function tp(lang, n, uk, en, vars) {
  const all = { ...(vars ?? {}), n };
  if (lang === "uk") return interpolate(pickForm("uk", n, uk) ?? uk.one, all);
  if (lang !== "en") {
    const f = pickForm(lang, n, entry(lang, uk.one));
    if (f) return interpolate(f, all);
  }
  return interpolate(pickForm("en", n, en) ?? en.other ?? en.one, all);
}
