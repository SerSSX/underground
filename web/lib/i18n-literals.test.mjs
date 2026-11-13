// TASK-303 G2 fix F4: no Ukrainian text reaches the UI outside tr/tf/tp. Every app/**/*.tsx file is read with
// comments blanked and every tr/tf/tp call blanked; any Cyrillic left is a string or JSX text a non-Ukrainian
// reader would see in Ukrainian. Known exceptions are listed below by file, with the reason.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { splitArgs, stripComments } from "../scripts/i18n-extract.mjs";

const WEB = join(dirname(fileURLToPath(import.meta.url)), "..");

/** Files allowed to hold Ukrainian literals. */
const ALLOWED = new Map([
  // Legal pages: Ukrainian only until a real translation (TASK-303 L1 notes, "Not localised yet").
  ["app/abuse/page.tsx", "legal page"],
  ["app/privacy/page.tsx", "legal page"],
  ["app/refunds/page.tsx", "legal page"],
  ["app/terms/page.tsx", "legal page"],
  // Server-rendered metadata (titles, descriptions, Open Graph): Ukrainian until per-language URLs.
  ["app/layout.tsx", "metadata"],
  ["app/page.tsx", "metadata"],
  ["app/account/layout.tsx", "metadata"],
  ["app/download/layout.tsx", "metadata"],
  ["app/inbox/layout.tsx", "metadata"],
  ["app/login/layout.tsx", "metadata"],
  ["app/me/tickets/layout.tsx", "metadata"],
  ["app/onboarding/layout.tsx", "metadata"],
  ["app/organizer/layout.tsx", "metadata"],
  ["app/register/layout.tsx", "metadata"],
  ["app/staff/layout.tsx", "metadata"],
  ["app/c/[category]/page.tsx", "metadata"],
  ["app/city/[city]/page.tsx", "metadata"],
  ["app/events/[eventId]/page.tsx", "metadata"],
]);

function walk(dir, out = []) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (e.name.endsWith(".tsx")) out.push(p);
  }
  return out;
}

/** Lines of `raw` that still hold Cyrillic once comments and tr/tf/tp calls are blanked. */
export function cyrillicOutsideCalls(raw) {
  const blank = stripComments(raw).split("");
  const src = blank.join("");
  const re = /(?<![\w$.])(tr|tf|tp)\(/g;
  let m;
  while ((m = re.exec(src))) {
    let end;
    try {
      ({ end } = splitArgs(src, m.index + m[0].length));
    } catch {
      continue;
    }
    for (let i = m.index; i <= end; i++) if (blank[i] !== "\n") blank[i] = " ";
  }
  const hits = [];
  blank.join("").split("\n").forEach((line, i) => {
    if (/[\u0400-\u04FF]/.test(line)) hits.push({ line: i + 1, text: raw.split("\n")[i].trim().slice(0, 100) });
  });
  return hits;
}

test("no Ukrainian literal in app/**/*.tsx outside tr/tf/tp (except the listed files)", () => {
  const found = [];
  for (const file of walk(join(WEB, "app"))) {
    const rel = relative(WEB, file).split("\\").join("/");
    if (ALLOWED.has(rel)) continue;
    for (const h of cyrillicOutsideCalls(readFileSync(file, "utf8"))) found.push(`${rel}:${h.line}: ${h.text}`);
  }
  assert.deepEqual(found, [], "wrap these in tr/tf/tp, or add the file to ALLOWED with a reason");
});

test("the allow-list names only files that exist", () => {
  for (const rel of ALLOWED.keys()) assert.ok(existsSync(join(WEB, rel)), `${rel} no longer exists: drop it from ALLOWED`);
});

test("the literal scan sees what it should", () => {
  assert.deepEqual(cyrillicOutsideCalls(`const a = tr(lang, "Так", "Yes");\n// Коментар\n/* блок */ const b = 1;`), []);
  assert.deepEqual(cyrillicOutsideCalls(`const a = tf(lang, "До {n} (так)", "Up to {n}", { n: f("x") });`), []);
  assert.equal(cyrillicOutsideCalls(`<label>Ім'я</label>`).length, 1, "JSX text");
  assert.equal(cyrillicOutsideCalls(`const x = "Київ";`).length, 1, "a string literal");
  assert.equal(cyrillicOutsideCalls(`const x = \`Події в \${city}\`;`).length, 1, "a template literal");
  assert.equal(cyrillicOutsideCalls(`setTr("Так")`).length, 1, "a look-alike function name is not a call site");
});
