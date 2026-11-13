// Chat sends one at a time, in order (TASK-305 review: a Send tap while a message was in flight was dropped).
// Types: lib/send-queue.d.mts. Tested in lib/send-queue.test.mjs.

/**
 * A queue that sends each enqueued item exactly once, one at a time, in the order they were enqueued. An item
 * enqueued while another is in flight waits and goes as soon as that one settles (the pump picks it up in its
 * `finally`). When a send fails, the items still waiting are not sent: they are handed to `onFailed` together
 * with the failed one, so the caller can give the text back to the user instead of losing it.
 * @template T, R
 * @param {{ send: (item: T) => Promise<R>; onSent?: (item: T, result: R) => void; onFailed?: (failed: T[], error: unknown) => void }} h
 */
export function createSendQueue(h) {
  /** @type {T[]} */
  const waiting = [];
  let inFlight = false;

  async function pump() {
    if (inFlight) return;
    const item = waiting.shift();
    if (item === undefined) return;
    inFlight = true;
    try {
      let ok = false;
      /** @type {R | undefined} */
      let res;
      let err;
      try {
        res = await h.send(item);
        ok = true;
      } catch (e) {
        err = e;
      }
      // The callbacks run outside the send's try: a throwing onSent must never report a sent item as failed
      // (the caller would hand its text back and the user would send it twice).
      if (ok) {
        try {
          h.onSent?.(item, /** @type {R} */ (res));
        } catch (e) {
          console.error(e); // a bug in the caller's display code; the item was sent
        }
      } else {
        h.onFailed?.([item, ...waiting.splice(0)], err);
      }
    } finally {
      inFlight = false;
      if (waiting.length) void pump();
    }
  }

  return {
    /** @param {T} item */
    enqueue(item) {
      waiting.push(item);
      void pump();
    },
    get busy() { return inFlight; },
    get waiting() { return waiting.length; },
  };
}
