"use client";

import { useEffect, useState } from "react";
import { unreadCounts, type UnreadCounts } from "./api";

/** Fired by a page that just read messages or alerts, so the badges catch up at once instead of on the next poll. */
export const UNREAD_CHANGED = "ug:unread-changed";
export function unreadChanged(): void {
  if (typeof window !== "undefined") window.dispatchEvent(new Event(UNREAD_CHANGED));
}

/** How often the badges are refreshed while the tab is visible. */
const POLL_MS = 30_000;

/**
 * Unread chat messages and alerts for the nav badges (`GET /api/me/unread`). Only for a signed-in user (`userId`);
 * refreshed every 30 s while the page is visible, when the window regains focus, on navigation (`path`) and when a
 * page reports it read something. Errors keep the last known counts.
 */
export function useUnread(userId: string | null | undefined, path: string): UnreadCounts | null {
  const [counts, setCounts] = useState<UnreadCounts | null>(null);
  useEffect(() => {
    if (!userId) {
      setCounts(null);
      return;
    }
    let stop = false;
    const pull = () => {
      if (document.visibilityState === "hidden") return;
      unreadCounts().then((c) => { if (!stop) setCounts(c); }).catch(() => {});
    };
    const onVisible = () => { if (document.visibilityState === "visible") pull(); };
    pull();
    const id = setInterval(pull, POLL_MS);
    window.addEventListener("focus", pull);
    window.addEventListener(UNREAD_CHANGED, pull);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      stop = true;
      clearInterval(id);
      window.removeEventListener("focus", pull);
      window.removeEventListener(UNREAD_CHANGED, pull);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [userId, path]);
  return counts;
}
