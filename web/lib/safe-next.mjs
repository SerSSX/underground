// Where to go after sign-in: a same-site path, or the fallback. (TASK-302 review M1.)
// "/\t/evil.example" passes a naive startsWith("/") check, and the URL parser strips the tab, turning it into
// "//evil.example" — another origin. So: no control characters, spaces or backslashes, no "//", and finally
// the parsed origin must equal ours.

/** @param {string|null|undefined} next @param {string} origin @param {string} [fallback] @returns {string} */
export function safeNextFor(next, origin, fallback = "/") {
  if (!next || typeof next !== "string" || !next.startsWith("/") || next.startsWith("//")) return fallback;
  for (let i = 0; i < next.length; i++) {
    const c = next.charCodeAt(i);
    if (c <= 0x20 || c === 0x5c || c === 0x7f) return fallback;
  }
  try {
    const u = new URL(next, origin);
    return u.origin === origin ? u.pathname + u.search + u.hash : fallback;
  } catch {
    return fallback;
  }
}
