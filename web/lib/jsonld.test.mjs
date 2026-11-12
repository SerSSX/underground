// node --test lib/  (runs in CI's web job)
import { test } from "node:test";
import assert from "node:assert/strict";
import { jsonLdString } from "./jsonld.mjs";

test("a title cannot close the JSON-LD script element", () => {
  const evil = "</script><script>alert(1)</script>";
  const out = jsonLdString({ name: evil });
  assert.ok(!out.toLowerCase().includes("</script"), out);
  assert.ok(!out.includes("<"), out);
});

test("the JSON is unchanged for a parser", () => {
  const sep = "a" + String.fromCharCode(0x2028) + "b" + String.fromCharCode(0x2029) + "c";
  const data = { name: "Рок & ролл <live>", url: "https://x/y?a=1&b=2", sep };
  const out = jsonLdString(data);
  assert.deepEqual(JSON.parse(out), data);
  assert.ok(!out.includes(String.fromCharCode(0x2028)), "raw line separator left in output");
});
