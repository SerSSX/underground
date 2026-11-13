// When an event is over, and how a user's tickets split into Upcoming and Past (TASK-305).
// Types: lib/event-time.d.mts. Tested in lib/event-time.test.mjs.

/** An event is over at its end time, or 12 h after the start when no end is set. Until then a guest can
 *  still show the QR and staff can still scan (a concert that started at 19:00 still admits at 19:05). */
export function isOver(startsAt, endsAt, now = Date.now()) {
  const start = startsAt ? Date.parse(startsAt) : NaN;
  let end = endsAt ? Date.parse(endsAt) : NaN;
  // An end at or before the start is bad data: ignore it rather than close the doors early.
  if (!Number.isFinite(end) || (Number.isFinite(start) && end <= start)) end = Number.isFinite(start) ? start + 12 * 3600_000 : NaN;
  return Number.isFinite(end) && end < now;
}

/** Ticket statuses that are still a place at the event: booked, being handed to someone, or already scanned in. */
export const LIVE_TICKET_STATUSES = ["active", "pending_claim", "used"];

/**
 * Upcoming: the event is not over and the user still holds a place there — a place already scanned at the door
 * counts, so a guest who checked in at 19:05 still finds the event under Upcoming (TASK-305). Everything else is
 * Past. Upcoming soonest first, Past newest first.
 * @template {{ startsAt: string | null; endsAt: string | null; tickets: { status: string }[] }} G
 * @param {G[]} groups
 * @returns {{ upcoming: G[]; past: G[] }}
 */
export function splitTicketGroups(groups, now = Date.now()) {
  const upcoming = [];
  const past = [];
  for (const g of groups) {
    const live = !isOver(g.startsAt, g.endsAt, now) && g.tickets.some((t) => LIVE_TICKET_STATUSES.includes(t.status));
    (live ? upcoming : past).push(g);
  }
  upcoming.sort((a, b) => (a.startsAt ?? "").localeCompare(b.startsAt ?? ""));
  past.sort((a, b) => (b.startsAt ?? "").localeCompare(a.startsAt ?? ""));
  return { upcoming, past };
}
