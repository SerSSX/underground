"use client";

import { useEffect } from "react";

/**
 * Blocks pinch and double-tap zoom on phones, at the owner's explicit request (TASK-305): iOS WebKit ignores the
 * viewport's user-scalable=no / maximum-scale, so the page could still be pinched. Double-tap zoom is stopped by
 * `touch-action: manipulation` in globals.css; pinch is stopped here.
 *
 * Accessibility cost, accepted by the owner: people with low vision cannot zoom the page in (WCAG 1.4.4 Resize
 * text). Browser-level text size settings and the OS zoom (iOS Zoom, Android magnification) still work.
 *
 * Only multi-finger moves are cancelled, so one-finger scrolling stays native. The listeners must be non-passive,
 * or preventDefault is ignored.
 */
export default function NoZoom() {
  useEffect(() => {
    const block = (e: Event) => e.preventDefault();
    // iOS-only gesture events (pinch / rotate in Safari and every iOS browser, which are all WebKit).
    const gestures = ["gesturestart", "gesturechange", "gestureend"] as const;
    const pinch = (e: TouchEvent) => {
      const scale = (e as TouchEvent & { scale?: number }).scale;
      if (e.touches.length > 1 || (typeof scale === "number" && scale !== 1)) e.preventDefault();
    };
    const opts: AddEventListenerOptions = { passive: false };
    for (const g of gestures) document.addEventListener(g, block, opts);
    document.addEventListener("touchmove", pinch, opts);
    return () => {
      for (const g of gestures) document.removeEventListener(g, block, opts);
      document.removeEventListener("touchmove", pinch, opts);
    };
  }, []);
  return null;
}
