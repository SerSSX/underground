// Pure geo helpers for the web (TASK-303 G2): place labels, distances, the city filter, the Google Maps
// link, the remembered city. No network, no DOM; lib/geo.ts does the requests, lib/geo.test.mjs tests this.
//
// Everything a geocoder returns is untrusted text: it is only ever rendered as React text or put in an
// input's value. A URL is built here only from numbers we checked or through encodeURIComponent.

/** Events whose point lies this close to the chosen city's point count as "in" it (a metro area). */
export const CITY_RADIUS_KM = 30;
/** An event that also carries the chosen city's name counts within this distance (a big city's edge);
 *  beyond it the same name is another place ("Springfield"). */
export const SAME_NAME_RADIUS_KM = 150;
const MAX_TEXT = 200;
const MAX_CITY = 100;

/** A finite latitude and longitude within range. @param {any} o @returns {boolean} */
export function hasPoint(o) {
  if (!o || typeof o.lat !== "number" || typeof o.lng !== "number") return false;
  return Number.isFinite(o.lat) && Number.isFinite(o.lng) && Math.abs(o.lat) <= 90 && Math.abs(o.lng) <= 180;
}

/** Great-circle distance in km (same formula as the API's tz.rs). */
export function haversineKm(lat1, lng1, lat2, lng2) {
  const rad = Math.PI / 180;
  const dp = (lat2 - lat1) * rad;
  const dl = (lng2 - lng1) * rad;
  const a = Math.sin(dp / 2) ** 2 + Math.cos(lat1 * rad) * Math.cos(lat2 * rad) * Math.sin(dl / 2) ** 2;
  return 2 * 6371 * Math.asin(Math.min(1, Math.sqrt(a)));
}

/** Round to `decimals` places (no "-0"): what leaves the browser is coarser than what it knows. */
export function roundCoord(v, decimals) {
  const f = 10 ** decimals;
  const r = Math.round(v * f) / f;
  return r === 0 ? 0 : r;
}

/** The UI language as the API's `lang` (two lowercase letters; the API maps it to Photon's set). */
export function geoLang(lang) {
  return typeof lang === "string" && /^[a-z]{2}$/.test(lang) ? lang : "en";
}

function text(v, max = MAX_TEXT) {
  if (typeof v !== "string") return null;
  // Control characters out, whitespace collapsed, length capped: a label, never markup.
  // eslint-disable-next-line no-control-regex
  const s = v.replace(/[\u0000-\u001f\u007f]/g, " ").replace(/\s+/g, " ").trim();
  return s ? s.slice(0, max) : null;
}

/** One API GeoPlace, re-checked: strings or null, a valid point, a known kind. Anything else → null. */
export function sanitizePlace(raw) {
  if (!raw || typeof raw !== "object" || !hasPoint(raw)) return null;
  const cc = text(raw.country_code, 2);
  const kind = raw.kind === "city" || raw.kind === "address" ? raw.kind : "place";
  const place = {
    label: text(raw.label) ?? "",
    name: text(raw.name),
    street: text(raw.street),
    housenumber: text(raw.housenumber, 20),
    city: text(raw.city, MAX_CITY),
    // OSM default-language names (TASK-303 G2 fix batch; absent from older APIs → null).
    name_default: text(raw.name_default, MAX_CITY),
    city_default: text(raw.city_default, MAX_CITY),
    state: text(raw.state),
    country: text(raw.country, 80),
    country_code: cc && /^[A-Za-z]{2}$/.test(cc) ? cc.toUpperCase() : null,
    lat: raw.lat,
    lng: raw.lng,
    kind,
  };
  return place.label || place.name || place.city ? place : null;
}

/** A list from the API → only well-formed places. */
export function sanitizePlaces(list) {
  return Array.isArray(list) ? list.map(sanitizePlace).filter(Boolean) : [];
}

function joinUnique(parts) {
  const out = [];
  for (const p of parts) {
    if (p && !out.some((x) => x.toLowerCase() === p.toLowerCase())) out.push(p);
  }
  return out.join(", ");
}

/** The city a place names, as the visitor reads it (their language when the geocoder has it): a city
 *  result's own name, else the locality it lies in. For labels and lists. */
export function cityName(place) {
  if (!place) return "";
  return (place.kind === "city" ? place.name ?? place.city : place.city) ?? "";
}

/** The city name to STORE (event city, prefs city, the home filter): the OSM default-language name, so an
 *  event made by an English reader ("Kyiv") and one made by a Ukrainian reader ("Київ") share one name.
 *  Falls back to the localized name when the API does not send default names. */
export function cityStoredName(place) {
  if (!place) return "";
  if (place.kind === "city") return place.city_default ?? place.name_default ?? place.city ?? place.name ?? "";
  return place.city_default ?? place.city ?? "";
}

/** "City, Region, Country" for a city suggestion. */
export function cityLabel(place) {
  return joinUnique([cityName(place) || place.name, place.state, place.country]) || place.label;
}

/** "Name, Street 12" for the venue field: no city, region or country (they have their own fields).
 *  A place with neither a name nor a street falls back to its full label without the country. */
export function addressLabel(place) {
  const street = place.street ? (place.housenumber ? `${place.street} ${place.housenumber}` : place.street) : null;
  const own = joinUnique([place.name, street]);
  const label = own || joinUnique([place.city, place.state]) || place.label;
  return label.slice(0, MAX_TEXT);
}

/** The second line of an address suggestion: where it is. */
export function addressWhere(place) {
  return joinUnique([place.city, place.state, place.country]);
}

function sameName(a, b) {
  return typeof a === "string" && typeof b === "string" && a.trim().toLowerCase() === b.trim().toLowerCase() && a.trim() !== "";
}

/** Is an event in the chosen city? When both have points: within CITY_RADIUS_KM, or the same name within
 *  SAME_NAME_RADIUS_KM (names alone fail across languages, "Kyiv"/"Київ", and across countries,
 *  "Springfield"). Without a point on either side: the same name (the stored or the shown one). */
export function inCity(event, chosen) {
  if (!chosen) return true;
  const named = sameName(event.city, chosen.name) || sameName(event.city, chosen.label);
  if (hasPoint(chosen) && hasPoint(event)) {
    const d = haversineKm(chosen.lat, chosen.lng, event.lat, event.lng);
    return d <= CITY_RADIUS_KM || (named && d <= SAME_NAME_RADIUS_KM);
  }
  return named;
}

/** Distance from `origin` to the event in km, or null when either has no point. */
export function distanceKm(event, origin) {
  return hasPoint(event) && hasPoint(origin) ? haversineKm(origin.lat, origin.lng, event.lat, event.lng) : null;
}

/** The whole number shown as "~{n} km": at least 1. */
export function kmShown(d) {
  return Math.max(1, Math.round(d));
}

/** Nearest first; events without a point keep their order after the rest. */
export function sortByDistance(events, origin) {
  return events
    .map((e, i) => ({ e, i, d: distanceKm(e, origin) }))
    .sort((a, b) => (a.d === null ? (b.d === null ? a.i - b.i : 1) : b.d === null ? -1 : a.d - b.d || a.i - b.i))
    .map((x) => x.e);
}

const MAPS = "https://www.google.com/maps/search/?api=1&query=";

/**
 * The "Open in Google Maps" link. Coordinates when the event has its own point; the address + city query
 * (as before) when it has none, or when its point is only its city's centre (copied from the gazetteer when
 * the address was typed, not picked) while it has an address — a pin in the middle of town would be wrong.
 * `cityCenter`: the gazetteer point of the event's city, `null` when the gazetteer has none, `undefined`
 * while unknown (then an event with an address keeps the address query).
 */
export function mapsHref(event, cityCenter) {
  const address = typeof event.venue_address === "string" ? event.venue_address.trim() : "";
  const city = typeof event.city === "string" ? event.city.trim() : "";
  const atCenter = cityCenter === undefined
    || (hasPoint(cityCenter) && Math.abs(cityCenter.lat - event.lat) < 1e-6 && Math.abs(cityCenter.lng - event.lng) < 1e-6);
  if (hasPoint(event) && !(address && atCenter)) {
    return `${MAPS}${event.lat.toFixed(6)},${event.lng.toFixed(6)}`;
  }
  const q = [address, city].filter(Boolean).join(", ");
  return q ? `${MAPS}${encodeURIComponent(q)}` : null;
}

/** The remembered home city (localStorage) → a chosen city; a legacy plain name still works. */
export function parseStoredCity(raw) {
  if (typeof raw !== "string" || !raw.trim()) return null;
  let v;
  try {
    v = JSON.parse(raw);
  } catch {
    const name = text(raw, MAX_CITY);
    return name ? { name } : null;
  }
  if (v === null || typeof v !== "object") {
    const name = text(raw, MAX_CITY);
    return name ? { name } : null;
  }
  const name = text(v.name, MAX_CITY);
  if (!name) return null;
  const out = { name };
  const label = text(v.label, MAX_CITY);
  if (label && label !== name) out.label = label;
  if (hasPoint(v)) Object.assign(out, { lat: v.lat, lng: v.lng });
  if (typeof v.country_code === "string" && /^[A-Z]{2}$/.test(v.country_code)) out.country_code = v.country_code;
  if (v.near === true && hasPoint(v)) out.near = true;
  return out;
}

/** A chosen city → what is remembered. A "near me" point is kept at ~1 km, never the exact position. */
export function storeCity(chosen) {
  const out = { name: chosen.name };
  if (chosen.label && chosen.label !== chosen.name) out.label = chosen.label;
  if (hasPoint(chosen)) {
    const d = chosen.near ? 2 : 4;
    Object.assign(out, { lat: roundCoord(chosen.lat, d), lng: roundCoord(chosen.lng, d) });
  }
  if (chosen.country_code) out.country_code = chosen.country_code;
  if (chosen.near) out.near = true;
  return JSON.stringify(out);
}

/**
 * The gazetteer row (`GET /api/cities`) a form may take a typed city's zone, point and country from
 * (TASK-303 G1 fix batch: learned `osm` rows answer only their own country). A row matches the name (any case)
 * and is either seeded (`source` "seed", or no `source` from an older API) or learned with the country code the
 * form already knows. Among several, the one in the known country wins. None → null: the caller falls back.
 */
export function trustedCityRow(rows, name, countryCode) {
  if (!Array.isArray(rows) || typeof name !== "string") return null;
  const lower = name.trim().toLowerCase();
  const cc = typeof countryCode === "string" && /^[A-Za-z]{2}$/.test(countryCode) ? countryCode.toUpperCase() : null;
  const same = rows.filter((r) => r && typeof r.name === "string" && r.name.trim().toLowerCase() === lower && hasPoint(r));
  const inCountry = (r) => !!cc && typeof r.country_code === "string" && r.country_code.toUpperCase() === cc;
  const seeded = (r) => r.source === undefined || r.source === null || r.source === "seed";
  return same.find(inCountry) ?? same.find(seeded) ?? null;
}
