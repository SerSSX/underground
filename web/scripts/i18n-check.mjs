#!/usr/bin/env node
// Check the translated catalogs against locales/_source.json (TASK-303 L1). No dependencies.
//
//   node scripts/i18n-check.mjs                  exit 1 on any problem, missing keys included — the bar CI holds
//                                                (lib/i18n.test.mjs runs the same check without allowMissing)
//   node scripts/i18n-check.mjs --allow-missing  missing keys are reported, not failed (local drafting only)
//
// A problem is: a catalog that is not a JSON object; a value of the wrong shape (text where the source is
// a plural object, or the reverse; an empty text); a plural without a form the language needs (its CLDR
// categories, e.g. pl: one/few/many/other, ja: other); a plural form the language does not have; or a
// {placeholder} that differs from the source. Keys the source no longer has are reported as stale (warning).

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const WEB = join(dirname(fileURLToPath(import.meta.url)), "..");
export const TARGET_LANGS = ["es", "pt", "fr", "de", "it", "pl", "tr", "ja"];

const PH = /\{([A-Za-z_][A-Za-z0-9_]*)\}/g;
const phSet = (s) => new Set([...String(s).matchAll(PH)].map((m) => m[1]));
const show = (set) => `{${[...set].sort().join("}, {")}}`;
const same = (a, b) => a.size === b.size && [...a].every((x) => b.has(x));

/** Integer plural categories a language uses (what n = 0…200 can select), e.g. pl → one, few, many. */
export function neededCategories(lang) {
  const r = new Intl.PluralRules(lang);
  const out = new Set();
  for (let n = 0; n <= 200; n++) out.add(r.select(n));
  return out;
}

/**
 * @param {{ allowMissing?: boolean, source?: Record<string, unknown>, catalogs?: Record<string, unknown> }} [opts]
 *   `source` / `catalogs` override the files (for tests).
 */
export function checkCatalogs(opts = {}) {
  const errors = [];
  const warnings = [];
  const missing = {};
  const source = opts.source ?? JSON.parse(readFileSync(join(WEB, "locales", "_source.json"), "utf8"));
  for (const lang of TARGET_LANGS) {
    let cat;
    if (opts.catalogs) cat = opts.catalogs[lang] ?? {};
    else {
      try {
        cat = JSON.parse(readFileSync(join(WEB, "locales", `${lang}.json`), "utf8"));
      } catch (e) {
        errors.push(`${lang}.json: cannot read: ${e.message}`);
        continue;
      }
    }
    if (!cat || typeof cat !== "object" || Array.isArray(cat)) {
      errors.push(`${lang}.json: must be a JSON object`);
      continue;
    }
    const need = neededCategories(lang);
    const allowed = new Set(new Intl.PluralRules(lang).resolvedOptions().pluralCategories);
    missing[lang] = 0;
    for (const [key, src] of Object.entries(source)) {
      if (!Object.prototype.hasOwnProperty.call(cat, key)) {
        missing[lang]++;
        if (!opts.allowMissing) errors.push(`${lang}.json: missing "${key}"`);
        continue;
      }
      const val = cat[key];
      if (typeof src === "string") {
        if (typeof val !== "string" || val.trim() === "") {
          errors.push(`${lang}.json: "${key}" must be a non-empty text`);
          continue;
        }
        const want = phSet(src);
        const got = phSet(val);
        if (!same(want, got)) errors.push(`${lang}.json: "${key}" has ${show(got)}, the source has ${show(want)}`);
      } else {
        if (!val || typeof val !== "object" || Array.isArray(val)) {
          errors.push(`${lang}.json: "${key}" is a plural: give an object of forms (${[...need].join(", ")})`);
          continue;
        }
        const want = new Set(Object.values(src).flatMap((v) => [...phSet(v)]));
        for (const c of need) if (typeof val[c] !== "string" || val[c].trim() === "") errors.push(`${lang}.json: "${key}" needs the "${c}" form`);
        for (const [c, form] of Object.entries(val)) {
          if (!allowed.has(c)) { errors.push(`${lang}.json: "${key}" has a "${c}" form, which ${lang} does not use`); continue; }
          if (typeof form !== "string") { errors.push(`${lang}.json: "${key}" "${c}" must be a text`); continue; }
          const got = phSet(form);
          // A form may leave out {n} ("one" → "un lugar"), but never invent or drop any other placeholder.
          const wantHere = new Set([...want].filter((p) => p !== "n" || got.has("n")));
          if (!same(wantHere, got)) errors.push(`${lang}.json: "${key}" "${c}" has ${show(got)}, the source has ${show(want)}`);
        }
      }
    }
    for (const key of Object.keys(cat)) {
      if (!Object.prototype.hasOwnProperty.call(source, key)) warnings.push(`${lang}.json: "${key}" is not in the source any more (stale)`);
    }
  }
  return { errors, warnings, missing };
}

const isMain = process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url;
if (isMain) {
  const allowMissing = process.argv.includes("--allow-missing");
  const { errors, warnings, missing } = checkCatalogs({ allowMissing });
  for (const w of warnings) console.warn(`warning: ${w}`);
  for (const e of errors) console.error(`error: ${e}`);
  const total = Object.keys(JSON.parse(readFileSync(join(WEB, "locales", "_source.json"), "utf8"))).length;
  console.log(TARGET_LANGS.map((l) => `${l}: ${total - (missing[l] ?? total)}/${total}`).join("  "));
  process.exit(errors.length ? 1 : 0);
}
