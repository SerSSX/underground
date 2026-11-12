// MVP mode (brief 2026-11-11): events are free, no payments anywhere in the UI. The payment code stays in
// its branch; set NEXT_PUBLIC_FREE_ONLY=0 at build time to show the money screens again.
export const FREE_ONLY = process.env.NEXT_PUBLIC_FREE_ONLY !== "0";

/** Most places one person may book for one event (owner decision 2026-11-11). */
export const MAX_PLACES = 5;

/** The mail service is connected: sign-in by an emailed one-time code and password-reset emails.
 *  Off until plan step 1 connects it — without it no email arrives, and the site must not offer what it
 *  cannot do. */
export const EMAIL_CODE_LOGIN = process.env.NEXT_PUBLIC_EMAIL_LOGIN === "1";
