#!/usr/bin/env node
// Collect every translatable string into web/locales/_source.json (TASK-303 L1). No dependencies.
//
//   node scripts/i18n-extract.mjs          write locales/_source.json
//   node scripts/i18n-extract.mjs --check  exit 1 if the file is out of date (what lib/i18n.test.mjs runs)
//
// Sources:
//  - tr(lang, "uk", "en"), tf(lang, "uk {x}", "en {x}", vars), tp(lang, n, { one, few, many }, { one, other }, vars)
//    calls in web/app and web/lib (.ts, .tsx, .mjs). Arguments must be literals; a call with computed text is
//    an error unless its line says "i18n-dynamic" (its keys must then come from a data module below).
//  - lib/categories.mjs (category labels) and lib/server-messages.mjs (texts the API sends).
//
// Output: { "<Ukrainian source>": "<English>" , "<Ukrainian 'one' form>": { "one": "...", "other": "..." } }
// — the same shape a translated catalog (locales/<lang>.json) has. Keys keep first-seen order (file by file),
// so related strings stay together for the translator.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const WEB = join(dirname(fileURLToPath(import.meta.url)), "..");
export const SOURCE_PATH = join(WEB, "locales", "_source.json");
const SKIP_FILES = new Set(["lib/i18n-core.mjs"]); // the definitions themselves

function walk(dir, out = []) {
  for (const e of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
    const p = join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (/\.(tsx?|mjs)$/.test(e.name) && !/\.test\.mjs$/.test(e.name) && !/\.d\.m?ts$/.test(e.name)) out.push(p);
  }
  return out;
}

/** Blank out comments (keeping offsets and newlines), leaving strings and template literals intact. */
export function stripComments(src) {
  const out = src.split("");
  let i = 0;
  const n = src.length;
  /** @type {string[]} */
  const stack = []; // "`" for a template literal, "{" for a ${ } expression inside one
  while (i < n) {
    const c = src[i];
    const inTemplate = stack[stack.length - 1] === "`";
    if (inTemplate) {
      if (c === "\\") { i += 2; continue; }
      if (c === "`") { stack.pop(); i++; continue; }
      if (c === "$" && src[i + 1] === "{") { stack.push("{"); i += 2; continue; }
      i++;
      continue;
    }
    if (c === '"' || c === "'") {
      i++;
      while (i < n && src[i] !== c && src[i] !== "\n") i += src[i] === "\\" ? 2 : 1;
      i++;
      continue;
    }
    if (c === "`") { stack.push("`"); i++; continue; }
    if (c === "{") { if (stack.length) stack.push("{"); i++; continue; }
    if (c === "}") { if (stack.length && stack[stack.length - 1] === "{") stack.pop(); i++; continue; }
    // A line comment starts at a line start or after whitespace/punctuation, never after ":" ("https://" in JSX text).
    if (c === "/" && src[i + 1] === "/" && (i === 0 || /[\s;,{}()]/.test(src[i - 1]))) {
      while (i < n && src[i] !== "\n") out[i++] = " ";
      continue;
    }
    if (c === "/" && src[i + 1] === "*") {
      while (i < n && !(src[i] === "*" && src[i + 1] === "/")) { if (src[i] !== "\n") out[i] = " "; i++; }
      if (i < n) { out[i] = " "; out[i + 1] = " "; i += 2; }
      continue;
    }
    i++;
  }
  return out.join("");
}

/** Split the arguments of a call starting right after "(": raw text per argument and the end offset. */
export function splitArgs(src, start) {
  const args = [];
  let depth = 0;
  let i = start;
  let cur = start;
  while (i < src.length) {
    const c = src[i];
    if (c === '"' || c === "'") {
      i++;
      while (i < src.length && src[i] !== c) i += src[i] === "\\" ? 2 : 1;
      i++;
      continue;
    }
    if (c === "`") {
      i++;
      let d = 0;
      while (i < src.length) {
        if (src[i] === "\\") { i += 2; continue; }
        if (d === 0 && src[i] === "`") break;
        if (src[i] === "$" && src[i + 1] === "{") { d++; i += 2; continue; }
        if (d > 0 && src[i] === "}") d--;
        i++;
      }
      i++;
      continue;
    }
    if (c === "(" || c === "[" || c === "{") depth++;
    else if (c === ")" || c === "]" || c === "}") {
      if (depth === 0) {
        args.push(src.slice(cur, i));
        return { args: args.map((a) => a.trim()).filter((a, k, all) => a !== "" || k < all.length - 1), end: i };
      }
      depth--;
    } else if (c === "," && depth === 0) {
      args.push(src.slice(cur, i));
      cur = i + 1;
    }
    i++;
  }
  throw new Error("unterminated call");
}

/** A string literal ("…", '…', or `…` without ${}) → its value; anything else → undefined. */
function stringLiteral(raw) {
  const m = /^(["'])([\s\S]*)\1$/.exec(raw);
  if (m) {
    const body = m[1] === "'" ? m[2].replace(/\\'/g, "'").replace(/"/g, '\\"') : m[2];
    try { return JSON.parse(`"${body}"`); } catch { return undefined; }
  }
  const t = /^`([^`$]*)`$/.exec(raw);
  return t ? t[1].replace(/\\`/g, "`").replace(/\\\\/g, "\\") : undefined;
}

/** { one: "…", few: "…" } with literal values → object; anything else → undefined. */
function formsLiteral(raw) {
  const m = /^\{([\s\S]*)\}$/.exec(raw);
  if (!m) return undefined;
  const { args } = splitArgs(m[1] + ")", 0);
  const out = {};
  for (const a of args) {
    const p = /^(?:([A-Za-z]+)|"([A-Za-z]+)")\s*:\s*([\s\S]+)$/.exec(a);
    if (!p) return undefined;
    const v = stringLiteral(p[3].trim());
    if (v === undefined) return undefined;
    out[p[1] ?? p[2]] = v;
  }
  return out;
}

const PH = /\{([A-Za-z_][A-Za-z0-9_]*)\}/g;
const phs = (s) => [...new Set([...String(s).matchAll(PH)].map((m) => m[1]))].sort().join(",");
const formsPhs = (f) => [...new Set(Object.values(f).flatMap((v) => [...String(v).matchAll(PH)].map((m) => m[1])))].sort().join(",");

/** Scan the code and the data modules. Returns { source, errors, dynamic, calls }. */
export async function extract() {
  /** @type {Record<string, string | Record<string, string>>} */
  const source = {};
  const errors = [];
  const warnings = [];
  let dynamic = 0;
  let calls = 0;
  const add = (key, value, where) => {
    if (Object.prototype.hasOwnProperty.call(source, key)) {
      if (typeof source[key] !== typeof value) {
        errors.push(`${where}: "${key}" is used both as a plural and as a plain text`);
      } else if (JSON.stringify(source[key]) !== JSON.stringify(value)) {
        // The English differs but other languages share one translation per Ukrainian source: tell, do not fail.
        warnings.push(`${where}: "${key}" → ${JSON.stringify(value)} here, ${JSON.stringify(source[key])} first; other languages get one translation`);
      }
      return;
    }
    source[key] = value;
  };

  const files = [...walk(join(WEB, "app")), ...walk(join(WEB, "lib"))];
  for (const file of files) {
    const rel = relative(WEB, file).split("\\").join("/");
    if (SKIP_FILES.has(rel)) continue;
    const raw = readFileSync(file, "utf8");
    const src = stripComments(raw);
    const re = /(?<![\w$.])(tr|tf|tp)\(/g;
    let m;
    while ((m = re.exec(src))) {
      const fn = m[1];
      const line = raw.slice(0, m.index).split("\n").length;
      const lineText = raw.split("\n")[line - 1] ?? "";
      const where = `${rel}:${line}`;
      let args;
      try {
        ({ args } = splitArgs(src, m.index + m[0].length));
      } catch {
        errors.push(`${where}: could not read the ${fn}() call`);
        continue;
      }
      // Skip function declarations ("function tr(lang, uk, en) {") and type-only mentions.
      if (/function\s+(tr|tf|tp)\s*\($/.test(src.slice(Math.max(0, m.index - 12), m.index + 3))) continue;
      calls++;
      if (fn === "tr" || fn === "tf") {
        const uk = stringLiteral(args[1] ?? "");
        const en = stringLiteral(args[2] ?? "");
        if (uk === undefined || en === undefined) {
          if (/i18n-dynamic/.test(lineText) || /i18n-dynamic/.test(raw.split("\n")[line - 2] ?? "")) { dynamic++; continue; }
          errors.push(`${where}: ${fn}() needs literal Ukrainian and English texts (or an "i18n-dynamic" note with a data module)`);
          continue;
        }
        if (fn === "tr" && (phs(uk) || phs(en))) errors.push(`${where}: tr() text has {placeholders} — use tf()`);
        if (phs(uk) !== phs(en)) errors.push(`${where}: placeholders differ: uk {${phs(uk)}} vs en {${phs(en)}}`);
        add(uk, en, where);
      } else {
        const uk = formsLiteral(args[2] ?? "");
        const en = formsLiteral(args[3] ?? "");
        if (!uk || !en) {
          errors.push(`${where}: tp() needs literal { one, few, many } and { one, other } objects`);
          continue;
        }
        for (const k of ["one", "few", "many"]) if (!uk[k]) errors.push(`${where}: tp() Ukrainian forms need "${k}"`);
        for (const k of ["one", "other"]) if (!en[k]) errors.push(`${where}: tp() English forms need "${k}"`);
        if (formsPhs(uk) !== formsPhs(en)) errors.push(`${where}: plural placeholders differ: uk {${formsPhs(uk)}} vs en {${formsPhs(en)}}`);
        if (uk.one) add(uk.one, en, where);
      }
    }
  }

  const { CATEGORY_LABELS } = await import(pathToFileURL(join(WEB, "lib", "categories.mjs")).href);
  for (const [uk, en] of Object.values(CATEGORY_LABELS)) add(uk, en, "lib/categories.mjs");
  const { SERVER_MESSAGES, SERVER_PATTERNS, SERVER_CODES } = await import(pathToFileURL(join(WEB, "lib", "server-messages.mjs")).href);
  for (const [uk, en] of Object.entries(SERVER_MESSAGES)) add(uk, en, "lib/server-messages.mjs");
  for (const p of SERVER_PATTERNS) {
    if (phs(p.uk) !== phs(p.en) || phs(p.uk) !== [...p.vars].sort().join(",")) errors.push(`lib/server-messages.mjs: pattern "${p.uk}" placeholders do not match its vars`);
    add(p.uk, p.en, "lib/server-messages.mjs");
  }
  for (const [code, uk] of Object.entries(SERVER_CODES)) {
    if (uk !== null && !Object.prototype.hasOwnProperty.call(SERVER_MESSAGES, uk)) errors.push(`lib/server-messages.mjs: code "${code}" maps to "${uk}", which SERVER_MESSAGES does not translate`);
  }
  return { source, errors, warnings, dynamic, calls };
}

export function serialize(source) {
  return JSON.stringify(source, null, 2) + "\n";
}

const isMain = process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url;
if (isMain) {
  const { source, errors, warnings, dynamic, calls } = await extract();
  for (const w of warnings) console.warn(`warning: ${w}`);
  for (const e of errors) console.error(`error: ${e}`);
  const text = serialize(source);
  const plurals = Object.values(source).filter((v) => typeof v === "object").length;
  if (process.argv.includes("--check")) {
    let current = "";
    try { current = readFileSync(SOURCE_PATH, "utf8"); } catch { /* missing */ }
    if (current !== text) {
      console.error("locales/_source.json is out of date: run `node scripts/i18n-extract.mjs`");
      process.exit(1);
    }
  } else {
    writeFileSync(SOURCE_PATH, text);
  }
  console.log(`${Object.keys(source).length} keys (${plurals} plural) from ${calls} calls; ${dynamic} dynamic call(s) fed by data modules`);
  if (errors.length) process.exit(1);
}
