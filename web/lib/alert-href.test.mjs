import test from "node:test";
import assert from "node:assert/strict";
import { alertHref } from "./alert-href.mjs";

const EV = "0b5e2c1a-1111-4222-8333-944445555666";

test("an alert's own path wins over its event", () => {
  assert.equal(alertHref({ link: "/g/" + "a".repeat(64), event_id: EV }), "/g/" + "a".repeat(64));
  assert.equal(alertHref({ link: "/organizer/" + EV, event_id: EV }), "/organizer/" + EV);
});

test("no link, or a link that is not a plain path here, falls back to the event", () => {
  for (const link of [null, undefined, "", "https://evil.example/g/x", "//evil.example", "/\\evil.example", "javascript:alert(1)",
    "/g/x?next=//evil", "/g/x#frag", "/a//b", " /g/x", "/" + "a".repeat(300), 42]) {
    assert.equal(alertHref({ link, event_id: EV }), `/events/${EV}`, String(link));
  }
});

test("nothing usable gives no link", () => {
  assert.equal(alertHref({ link: "https://evil.example", event_id: "../x" }), null);
  assert.equal(alertHref({}), null);
  assert.equal(alertHref(null), null);
});
