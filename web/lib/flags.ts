// MVP mode (brief 2026-11-11): events are free, no payments anywhere in the UI. The payment code stays in
// its branch; set NEXT_PUBLIC_FREE_ONLY=0 at build time to show the money screens again.
export const FREE_ONLY = process.env.NEXT_PUBLIC_FREE_ONLY !== "0";

/** Most places one person may book for one event (owner decision 2026-11-11). */
export const MAX_PLACES = 5;

// Mail (code sign-in, password reset) is no longer a build-time flag: lib/features.ts reads it at runtime from
// GET /api/features (TASK-304).
