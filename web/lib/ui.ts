// Presentation helpers shared by the redesigned pages (TASK-302): category labels and colours,
// dates, initials. Pure functions, no network. Words and dates go through lib/i18n-core.mjs and
// lib/dates.mjs (TASK-303 L1: 10 languages).
import { tr, tp, type Lang } from "./i18n-core.mjs";
import { CATEGORY_LABELS, categoryFallback } from "./categories.mjs";
import { zonedParts, zoneLabel } from "./dates.mjs";

export { dateBadge, shortWhen, longDate, timeOf } from "./dates.mjs";

export function categoryLabel(cat: string | null | undefined, lang: Lang): string {
  if (!cat) return "";
  const key = cat.trim().toLowerCase();
  const l = Object.prototype.hasOwnProperty.call(CATEGORY_LABELS, key) ? CATEGORY_LABELS[key] : undefined;
  // i18n-dynamic: these keys reach locales/_source.json from lib/categories.mjs.
  return l ? tr(lang, l[0], l[1]) : categoryFallback(cat);
}

/** The handful shown as quick chips on the home page; everything else is behind "Усі категорії". */
export const TOP_CATEGORIES = [
  "concert", "party", "electronic", "dj set", "jazz", "rock", "stand-up", "theatre", "exhibition",
  "lecture", "workshop", "networking", "sport", "food & drink", "family", "festival",
];

const HUES: Record<string, number> = {
  rock: 4, metal: 350, jazz: 38, electronic: 265, "dj set": 280, party: 320, "hip-hop": 290, concert: 225,
  poetry: 190, literature: 200, yoga: 150, run: 140, hike: 120, sport: 130, cycling: 160, exhibition: 28,
  theatre: 340, cinema: 230, lecture: 210, workshop: 175, course: 185, networking: 215, startup: 245,
  "food & drink": 22, tasting: 12, family: 50, festival: 300, "board games": 170, quiz: 60, dance: 310,
};

/** Background for a poster placeholder: a gradient in the category's hue. */
export function categoryGradient(cat: string | null | undefined): string {
  const key = (cat ?? "").trim().toLowerCase();
  let hue = HUES[key];
  if (hue === undefined) {
    hue = 0;
    for (const ch of key) hue = (hue * 31 + ch.charCodeAt(0)) % 360;
  }
  return `radial-gradient(120% 90% at 0% 0%, hsl(${hue} 80% 58% / .9), transparent 60%), linear-gradient(150deg, hsl(${hue} 55% 32%), hsl(${(hue + 40) % 360} 45% 12%))`;
}

export type When = "all" | "today" | "tomorrow" | "weekend" | "week";

/** Does an event start inside the chosen window (local time)? */
export function inWindow(iso: string | null | undefined, w: When, now = new Date(), tz?: string | null): boolean {
  if (w === "all") return true;
  // Calendar days in the event's zone, so the filter agrees with the date printed on the card.
  const x = zonedParts(iso, tz);
  const n = zonedParts(now.toISOString(), tz);
  if (!x || !n) return false;
  const day = 86_400_000;
  const dayIdx = Math.round((Date.UTC(x.y, x.m, x.day) - Date.UTC(n.y, n.m, n.day)) / day);
  if (w === "today") return dayIdx === 0;
  if (w === "tomorrow") return dayIdx === 1;
  if (w === "week") return dayIdx >= 0 && dayIdx < 7;
  // weekend: the coming Saturday and Sunday (today included if it is one)
  const dow = n.dow;
  const toSat = dow === 0 ? -1 : 6 - dow;
  return dayIdx === toSat || dayIdx === toSat + 1;
}

const COUNTRY_TZ: Record<string, string> = {
  "україна": "Europe/Kyiv", ukraine: "Europe/Kyiv", poland: "Europe/Warsaw", "польща": "Europe/Warsaw",
  germany: "Europe/Berlin", "німеччина": "Europe/Berlin", france: "Europe/Paris", "франція": "Europe/Paris",
  spain: "Europe/Madrid", "іспанія": "Europe/Madrid", italy: "Europe/Rome", "італія": "Europe/Rome",
  portugal: "Europe/Lisbon", "португалія": "Europe/Lisbon", turkey: "Europe/Istanbul", "türkiye": "Europe/Istanbul",
  "туреччина": "Europe/Istanbul", japan: "Asia/Tokyo", "японія": "Asia/Tokyo", "united kingdom": "Europe/London",
  "велика британія": "Europe/London", georgia: "Asia/Tbilisi", "грузія": "Asia/Tbilisi", czechia: "Europe/Prague",
  "czech republic": "Europe/Prague", "чехія": "Europe/Prague", netherlands: "Europe/Amsterdam", austria: "Europe/Vienna",
  moldova: "Europe/Chisinau", "молдова": "Europe/Chisinau", romania: "Europe/Bucharest", lithuania: "Europe/Vilnius",
  latvia: "Europe/Riga", estonia: "Europe/Tallinn", slovakia: "Europe/Bratislava", hungary: "Europe/Budapest",
  belgium: "Europe/Brussels", ireland: "Europe/Dublin", denmark: "Europe/Copenhagen", sweden: "Europe/Stockholm",
  norway: "Europe/Oslo", finland: "Europe/Helsinki", greece: "Europe/Athens", croatia: "Europe/Zagreb",
  bulgaria: "Europe/Sofia", switzerland: "Europe/Zurich",
};

/** The time zone an event in this country runs on (single-zone countries only; null means: fall back). */
export function countryTz(country: string | null | undefined): string | null {
  return (country && COUNTRY_TZ[country.trim().toLowerCase()]) || null;
}

/** A zone's name in the reader's language (Intl generic name, else the IANA city): lib/dates.mjs zoneLabel. */
export function tzLabel(tz: string, lang: Lang): string {
  return zoneLabel(tz, lang);
}

export function initials(name: string | null | undefined): string {
  const parts = (name ?? "").trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "U";
  return (parts[0][0] + (parts[1]?.[0] ?? "")).toUpperCase();
}

/** "1 місце" / "5 місць" / "2 places" / "2 miejsca"… */
export function placesLabel(n: number, lang: Lang): string {
  return tp(lang, n, { one: "{n} місце", few: "{n} місця", many: "{n} місць" }, { one: "{n} place", other: "{n} places" });
}
