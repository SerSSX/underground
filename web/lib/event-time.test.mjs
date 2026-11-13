import test from "node:test";
import assert from "node:assert/strict";
import { isOver, splitTicketGroups } from "./event-time.mjs";

const NOW = Date.parse("2026-11-14T20:00:00Z");
const H = 3600_000;
const iso = (ms) => new Date(ms).toISOString();
const g = (id, startsAt, endsAt, ...statuses) => ({ id, startsAt, endsAt, tickets: statuses.map((status) => ({ status })) });

test("isOver: at the end, or 12 h after the start without one; a bad end is ignored", () => {
  assert.equal(isOver(iso(NOW - H), iso(NOW + H), NOW), false, "running");
  assert.equal(isOver(iso(NOW - 3 * H), iso(NOW - H), NOW), true, "ended");
  assert.equal(isOver(iso(NOW - 11 * H), null, NOW), false, "no end: 12 h grace");
  assert.equal(isOver(iso(NOW - 13 * H), null, NOW), true);
  assert.equal(isOver(iso(NOW - 2 * H), iso(NOW - 3 * H), NOW), false, "end before start → start + 12 h");
  assert.equal(isOver(null, null, NOW), false, "no dates: never over");
});

test("a checked-in place of an event that has not ended stays under Upcoming (TASK-305)", () => {
  const { upcoming, past } = splitTicketGroups([g("tonight", iso(NOW - H), iso(NOW + 2 * H), "used")], NOW);
  assert.deepEqual(upcoming.map((x) => x.id), ["tonight"]);
  assert.deepEqual(past, []);
});

test("split: status and time together decide, upcoming soonest first and past newest first", () => {
  const { upcoming, past } = splitTicketGroups([
    g("later", iso(NOW + 48 * H), null, "active"),
    g("soon", iso(NOW + 2 * H), null, "pending_claim"),
    g("over-used", iso(NOW - 30 * H), iso(NOW - 27 * H), "used"),
    g("over-active", iso(NOW - 50 * H), null, "active"),
    g("cancelled-only", iso(NOW + 5 * H), null, "cancelled"),
  ], NOW);
  assert.deepEqual(upcoming.map((x) => x.id), ["soon", "later"]);
  assert.deepEqual(past.map((x) => x.id), ["cancelled-only", "over-used", "over-active"], "no live place → Past");
});
