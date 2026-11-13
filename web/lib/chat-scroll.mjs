// Chat scrolling (TASK-304 review). Types: lib/chat-scroll.d.mts. Tested in lib/chat-scroll.test.mjs.

/** How close to the newest message still counts as "at the newest message", in CSS pixels. */
export const NEAR_BOTTOM_PX = 80;

/** Whether a scroll position is within NEAR_BOTTOM_PX of the end of the list. */
export function isNearBottom(scrollHeight, scrollTop, clientHeight) {
  return scrollHeight - scrollTop - clientHeight <= NEAR_BOTTOM_PX;
}
