// Geocoder client (TASK-303 G2): our API's /api/geo/* endpoints, never the geocoder itself. Every answer is
// re-checked by sanitizePlaces (lib/geo-core.mjs) and rendered as text only.
import { apiGet } from "./api";
import { geoLang, roundCoord, sanitizePlaces, type GeoPlace, type Point } from "./geo-core.mjs";

export type { GeoPlace, Point, ChosenCity } from "./geo-core.mjs";
export type GeoKind = "city" | "address";

/** Suggestions for `q` (2..120 characters). `bias`: a point to prefer results near (sent at ~11 km). */
export async function searchPlaces(q: string, kind: GeoKind, lang: string, bias?: Point | null, signal?: AbortSignal): Promise<GeoPlace[]> {
  const p = new URLSearchParams({ q: q.replace(/\s+/g, " ").trim().slice(0, 120), kind, lang: geoLang(lang) });
  if (bias) {
    p.set("lat", String(roundCoord(bias.lat, 1)));
    p.set("lng", String(roundCoord(bias.lng, 1)));
  }
  return sanitizePlaces(await apiGet<unknown>(`/api/geo/search?${p}`, signal));
}

/** The place at a point ("near me"). The point leaves the browser rounded to ~1 km. */
export async function reversePlace(lat: number, lng: number, lang: string, signal?: AbortSignal): Promise<GeoPlace | null> {
  const p = new URLSearchParams({ lat: String(roundCoord(lat, 2)), lng: String(roundCoord(lng, 2)), lang: geoLang(lang) });
  return sanitizePlaces(await apiGet<unknown>(`/api/geo/reverse?${p}`, signal))[0] ?? null;
}

/** The IANA zone a new event at this point gets on the API (the same rule as event create). */
export async function zoneFor(lat: number, lng: number, countryCode?: string | null, signal?: AbortSignal): Promise<string | null> {
  const p = new URLSearchParams({ lat: String(lat), lng: String(lng) });
  if (countryCode && /^[A-Za-z]{2}$/.test(countryCode)) p.set("country_code", countryCode);
  const r = await apiGet<{ timezone?: unknown }>(`/api/geo/timezone?${p}`, signal);
  return typeof r?.timezone === "string" && isZone(r.timezone) ? r.timezone : null;
}

/** A zone this browser can format in (an unknown name would make every date throw). */
export function isZone(tz: string): boolean {
  try {
    new Intl.DateTimeFormat("en", { timeZone: tz });
    return true;
  } catch {
    return false;
  }
}
