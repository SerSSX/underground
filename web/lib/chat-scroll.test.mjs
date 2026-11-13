import test from "node:test";
import assert from "node:assert/strict";
import { isNearBottom } from "./chat-scroll.mjs";

test("only a reader within 80 px of the newest message is kept there", () => {
  assert.equal(isNearBottom(2000, 1600, 400), true, "at the end");
  assert.equal(isNearBottom(2000, 1520, 400), true, "80 px above");
  assert.equal(isNearBottom(2000, 1519, 400), false, "81 px above");
  assert.equal(isNearBottom(2000, 300, 400), false, "reading older messages");
  assert.equal(isNearBottom(300, 0, 400), true, "a short list is always at its end");
});
