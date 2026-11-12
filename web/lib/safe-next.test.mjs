import { test } from "node:test";
import assert from "node:assert/strict";
import { safeNextFor } from "./safe-next.mjs";

const O = "https://underground-dev.example";

test("keeps same-site paths, query and fragment", () => {
  assert.equal(safeNextFor("/events/abc?x=1#y", O), "/events/abc?x=1#y");
  assert.equal(safeNextFor("/staff/accept", O), "/staff/accept");
});

test("refuses everything that can leave the site", () => {
  for (const bad of ["//evil.example", "/\t/evil.example", "/\n/evil.example", "/ /evil.example", "/\\evil.example",
                     "/\\/evil.example", "https://evil.example", "javascript:alert(1)", "evil.example", "", null, undefined]) {
    assert.equal(safeNextFor(bad, O, "/"), "/", JSON.stringify(bad));
  }
});
