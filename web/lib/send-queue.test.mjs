import test from "node:test";
import assert from "node:assert/strict";
import { createSendQueue } from "./send-queue.mjs";

/** A send whose calls stay pending until the test settles them. */
function controlled() {
  const calls = [];
  const send = (item) => new Promise((resolve, reject) => calls.push({ item, resolve, reject }));
  return { calls, send };
}
const tick = () => new Promise((r) => setTimeout(r, 0));

test("a second send while one is in flight is queued and goes after it, each exactly once", async () => {
  const c = controlled();
  const sent = [];
  const q = createSendQueue({ send: c.send, onSent: (item) => sent.push(item) });
  q.enqueue("first");
  q.enqueue("second");
  await tick();
  assert.deepEqual(c.calls.map((x) => x.item), ["first"], "only one request at a time");
  assert.equal(q.busy, true);
  assert.equal(q.waiting, 1);
  c.calls[0].resolve("ok1");
  await tick();
  assert.deepEqual(c.calls.map((x) => x.item), ["first", "second"], "the queued one fires when the first settles");
  c.calls[1].resolve("ok2");
  await tick();
  assert.deepEqual(sent, ["first", "second"]);
  assert.equal(c.calls.length, 2, "no double send");
  assert.equal(q.busy, false);
});

test("a failure hands back the failed item and everything still waiting, and sends none of them", async () => {
  const c = controlled();
  let failed = null;
  const q = createSendQueue({ send: c.send, onFailed: (items) => { failed = items; } });
  q.enqueue("a");
  q.enqueue("b");
  q.enqueue("c");
  await tick();
  c.calls[0].reject(new Error("offline"));
  await tick();
  assert.deepEqual(failed, ["a", "b", "c"]);
  assert.equal(c.calls.length, 1, "b and c were not sent");
  assert.equal(q.busy, false);
  q.enqueue("d");
  await tick();
  assert.deepEqual(c.calls.map((x) => x.item), ["a", "d"], "the queue works again after a failure");
});

test("a throwing onSent does not report the sent item as failed", async () => {
  const c = controlled();
  let failed = null;
  const quiet = console.error;
  console.error = () => {};
  const q = createSendQueue({ send: c.send, onSent: () => { throw new Error("render bug"); }, onFailed: (items) => { failed = items; } });
  q.enqueue("x");
  q.enqueue("y");
  await tick();
  c.calls[0].resolve("ok");
  await tick();
  console.error = quiet;
  assert.equal(failed, null);
  assert.equal(q.busy, true, "y is in flight");
  assert.deepEqual(c.calls.map((x) => x.item), ["x", "y"]);
});
