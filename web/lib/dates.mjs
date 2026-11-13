// Locale-aware dates for every UI language (TASK-303 L1). Intl.DateTimeFormat does the wording and the
// order ("пт, 20 лист." / "11月20日(金)" / "20 Kas Cum"); no month tables, no "day month" string surgery.
// Times are shown in the event's own zone, so a Kyiv concert reads 19:00 wherever the visitor is.
// Types: lib/dates.d.mts.

import { INTL_LOCALES } from "./i18n-core.mjs";

/** @type {Map<string, Intl.DateTimeFormat>} */
const cache = new Map();

/** A formatter for lang in tz; an unknown zone falls back to the visitor's local time (as before). */
function dtf(lang, opts, tz) {
  const locale = INTL_LOCALES[lang] ?? lang;
  const key = `${locale}|${tz ?? ""}|${JSON.stringify(opts)}`;
  let f = cache.get(key);
  if (!f) {
    try {
      f = new Intl.DateTimeFormat(locale, tz ? { ...opts, timeZone: tz } : opts);
    } catch {
      f = new Intl.DateTimeFormat(locale, opts);
    }
    cache.set(key, f);
  }
  return f;
}

function toDate(iso) {
  if (!iso) return null;
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? null : d;
}

const DOWS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/** Calendar parts of an instant in a time zone; without a zone (or with an unknown one), local time. */
export function zonedParts(iso, tz) {
  const x = toDate(iso);
  if (!x) return null;
  if (tz) {
    try {
      const f = new Intl.DateTimeFormat("en-US", { timeZone: tz, year: "numeric", month: "numeric", day: "numeric", hour: "numeric", minute: "numeric", weekday: "short", hourCycle: "h23" });
      const p = Object.fromEntries(f.formatToParts(x).map((q) => [q.type, q.value]));
      return { y: +p.year, m: +p.month - 1, day: +p.day, hh: +p.hour % 24, mi: +p.minute, dow: DOWS.indexOf(p.weekday) };
    } catch { /* unknown zone: fall through to local */ }
  }
  return { y: x.getFullYear(), m: x.getMonth(), day: x.getDate(), hh: x.getHours(), mi: x.getMinutes(), dow: x.getDay() };
}

/** "19:00" — 24-hour clock in the zone. */
export function timeOf(iso, tz) {
  const x = zonedParts(iso, tz);
  return x ? `${String(x.hh).padStart(2, "0")}:${String(x.mi).padStart(2, "0")}` : "";
}

const trimDot = (s) => s.replace(/\.$/, "");

/** { day: "20", mon: "лист" } for the date badge on cards (ja: { day: "20", mon: "11月" }). */
export function dateBadge(iso, lang, tz) {
  const x = toDate(iso);
  if (!x) return null;
  const day = dtf(lang, { day: "numeric" }, tz).formatToParts(x).find((p) => p.type === "day")?.value ?? "";
  const mon = trimDot(dtf(lang, { month: "short" }, tz).format(x));
  return { day, mon };
}

/** "пт, 20 лист · 19:00" / "Fri 20 Nov · 19:00" / "11月20日(金) · 19:00". */
export function shortWhen(iso, lang, tz) {
  const x = toDate(iso);
  if (!x) return "";
  return `${trimDot(dtf(lang, { weekday: "short", day: "numeric", month: "short" }, tz).format(x))} · ${timeOf(iso, tz)}`;
}

/** "пʼятниця, 20 листопада" / "Friday 20 November" / "11月20日金曜日" / "20 Kasım Cuma". */
export function longDate(iso, lang, tz) {
  const x = toDate(iso);
  if (!x) return "";
  const parts = dtf(lang, { weekday: "long", day: "numeric", month: "long" }, tz).formatToParts(x);
  if (lang === "uk") {
    // CLDR gives the Ukrainian weekday in the accusative here ("пʼятницю, 20 листопада", as in "у пʼятницю");
    // a heading wants the nominative, which the weekday-only format gives. Same order, one part swapped.
    const nominative = dtf(lang, { weekday: "long" }, tz).format(x);
    return parts.map((p) => (p.type === "weekday" ? nominative : p.value)).join("");
  }
  return parts.map((p) => p.value).join("");
}

/** A calendar date written in an ISO string, as is ("2026-06-15" or "2026-06-15T19:00…"): "15 черв · 19:00".
 *  No zone conversion: the string's own date and clock time are shown. */
export function isoDateLabel(iso, lang) {
  if (!iso || iso.length < 10) return "";
  const y = Number(iso.slice(0, 4));
  const m = Number(iso.slice(5, 7));
  const d = Number(iso.slice(8, 10));
  if (!y || !m || !d) return "";
  const date = trimDot(dtf(lang, { day: "numeric", month: "short" }, "UTC").format(new Date(Date.UTC(y, m - 1, d))));
  return iso.length >= 16 ? `${date} · ${iso.slice(11, 16)}` : date;
}

/** A zone's name for people, in their language (TASK-303 G2 fix F4): Intl's generic name ("Eastern European
 *  Time", "日本標準時", "за східноєвропейським часом"). Where Intl has no name (it would print "GMT+3") or does
 *  not support generic names, the IANA city ("Buenos Aires"). */
export function zoneLabel(tz, lang) {
  const city = (String(tz).split("/").pop() ?? String(tz)).replace(/_/g, " ");
  try {
    const part = new Intl.DateTimeFormat(INTL_LOCALES[lang] ?? lang, { timeZone: tz, timeZoneName: "longGeneric" })
      .formatToParts(new Date())
      .find((p) => p.type === "timeZoneName");
    const name = part?.value?.trim();
    return name && !/^(GMT|UTC)\b/.test(name) && !/\d/.test(name) ? name : city;
  } catch {
    return city;
  }
}
