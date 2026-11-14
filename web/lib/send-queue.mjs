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

/**
 * Unsent texts kept per conversation (TASK-305 round 3): when a send fails, the texts of conversations that are
 * not open are kept here, oldest first, and handed back when that conversation is opened again, so switching
 * chats never loses them.
 * @param {Map<string, string>} drafts conversation (peer id) → kept text
 * @param {{ peerId: string; body: string }[]} failed the failed items, oldest first
 * @param {string | null | undefined} openPeerId the conversation open now (its texts are not stashed)
 * @returns {string} the open conversation's own failed texts, joined, for the caller to put back in the field
 */
export function stashFailed(drafts, failed, openPeerId) {
  const here = [];
  for (const o of failed) {
    if (o.peerId === openPeerId) {
      here.push(o.body);
    } else {
      const prev = drafts.get(o.peerId);
      drafts.set(o.peerId, prev ? `${prev} ${o.body}` : o.body);
    }
  }
  return here.join(" ");
}

/**
 * The kept text of a conversation, removed from `drafts` (it goes back into the field once).
 * @param {Map<string, string>} drafts
 * @param {string} peerId
 * @returns {string}
 */
export function takeDraft(drafts, peerId) {
  const d = drafts.get(peerId) ?? "";
  drafts.delete(peerId);
  return d;
}
