import { test } from "node:test";
import assert from "node:assert/strict";
import { csvField, toCsv } from "./csv.mjs";

test("formula-looking names are neutralised", () => {
  for (const evil of ["=HYPERLINK(\"http://x\",\"y\")", "+1+1", "-2", "@SUM(A1)", "\tx", "\rx"]) {
    const f = csvField(evil);
    assert.ok(f.startsWith("\"'"), f);
  }
});

test("commas, quotes and newlines stay inside one cell", () => {
  const out = toCsv([["Імʼя", "Місць"], ['Олена "Оля", Київ\nдругий рядок', 3]]);
  assert.equal(out, '"Імʼя","Місць"\r\n"Олена ""Оля"", Київ\nдругий рядок","3"\r\n');
});
