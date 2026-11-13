// Pure helpers for uploaded pictures (TASK-304). Types: lib/images.d.mts. Tested in lib/images.test.mjs.

/** Longest side the API stores; the browser shrinks to this before uploading. */
export const MAX_SIDE = 1600;
/** Files at or under this size go up as they are (the API re-encodes them anyway). */
export const SHRINK_OVER_BYTES = 1_500_000;
const TYPES = new Set(["image/jpeg", "image/png", "image/webp"]);
const UPLOAD = /^(.*\/uploads\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\.webp$/i;

/** The 480 px card copy of a picture the API stored (`…/uploads/<uuid>.webp` → `…/uploads/<uuid>-480.webp`).
 *  Anything else — an older upload (.jpg/.png), an external link, a URL with a query — has none: null. */
export function smallVariant(url) {
  if (typeof url !== "string") return null;
  const m = UPLOAD.exec(url.trim());
  return m ? `${m[1]}-480.webp` : null;
}

/** Whether to shrink a picture in the browser before uploading, and to what size. null = send it as it is. */
export function shrinkPlan({ size, type, width, height }) {
  if (!TYPES.has(type) || !(width > 0) || !(height > 0)) return null;
  const long = Math.max(width, height);
  if (size <= SHRINK_OVER_BYTES && long <= MAX_SIDE) return null;
  const scale = Math.min(1, MAX_SIDE / long);
  return { width: Math.max(1, Math.round(width * scale)), height: Math.max(1, Math.round(height * scale)) };
}
