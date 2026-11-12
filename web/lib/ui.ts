// Presentation helpers shared by the redesigned pages (TASK-302): category labels and colours,
// dates, initials. Pure functions, no network.
import type { Lang } from "./i18n";

/** Category key → [uk, en] label. Keys are lower-case; events store either case ("Rock" / "rock"). */
const CATEGORY_LABELS: Record<string, [string, string]> = {
  concert: ["Концерт", "Concert"], rock: ["Рок", "Rock"], metal: ["Метал", "Metal"],
  electronic: ["Електроніка", "Electronic"], "hip-hop": ["Хіп-хоп", "Hip-hop"], jazz: ["Джаз", "Jazz"],
  pop: ["Поп", "Pop"], folk: ["Фолк", "Folk"], classical: ["Класика", "Classical"], "dj set": ["DJ-сет", "DJ set"],
  festival: ["Фестиваль", "Festival"], karaoke: ["Караоке", "Karaoke"], "stand-up": ["Стендап", "Stand-up"],
  theatre: ["Театр", "Theatre"], improv: ["Імпровізація", "Improv"], dance: ["Танці", "Dance"],
  poetry: ["Поезія", "Poetry"], cinema: ["Кіно", "Cinema"], exhibition: ["Виставка", "Exhibition"],
  photography: ["Фото", "Photography"], fashion: ["Мода", "Fashion"], literature: ["Література", "Literature"],
  craft: ["Ремесла", "Craft"], lecture: ["Лекція", "Lecture"], conference: ["Конференція", "Conference"],
  workshop: ["Майстер-клас", "Workshop"], course: ["Курс", "Course"], networking: ["Нетворкінг", "Networking"],
  startup: ["Стартапи", "Startups"], hackathon: ["Хакатон", "Hackathon"], sport: ["Спорт", "Sport"],
  yoga: ["Йога", "Yoga"], hike: ["Похід", "Hike"], run: ["Біг", "Running"], cycling: ["Велоспорт", "Cycling"],
  extreme: ["Екстрим", "Extreme"], party: ["Вечірка", "Party"], meetup: ["Зустріч", "Meetup"],
  charity: ["Благодійність", "Charity"], family: ["Для родини", "Family"], "food & drink": ["Їжа й напої", "Food & drink"],
  tasting: ["Дегустація", "Tasting"], "board games": ["Настільні ігри", "Board games"], quiz: ["Квіз", "Quiz"],
  market: ["Маркет", "Market"], gaming: ["Ігри", "Gaming"], spiritual: ["Духовне", "Spiritual"], other: ["Інше", "Other"],
};

export function categoryLabel(cat: string | null | undefined, lang: Lang): string {
  if (!cat) return "";
  const l = CATEGORY_LABELS[cat.trim().toLowerCase()];
  return l ? l[lang === "en" ? 1 : 0] : cat;
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

const MONTHS_SHORT: Record<Lang, string[]> = {
  uk: ["січ", "лют", "бер", "кві", "тра", "чер", "лип", "сер", "вер", "жов", "лис", "гру"],
  en: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
};
const MONTHS_LONG_GEN: Record<Lang, string[]> = {
  uk: ["січня", "лютого", "березня", "квітня", "травня", "червня", "липня", "серпня", "вересня", "жовтня", "листопада", "грудня"],
  en: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
};
const WEEKDAYS: Record<Lang, string[]> = {
  uk: ["неділя", "понеділок", "вівторок", "середа", "четвер", "пʼятниця", "субота"],
  en: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
};
const WEEKDAYS_SHORT: Record<Lang, string[]> = {
  uk: ["нд", "пн", "вт", "ср", "чт", "пт", "сб"],
  en: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
};

type Parts = { y: number; m: number; day: number; hh: number; mi: number; dow: number };

/** Calendar parts of an instant in a time zone (the event's own, so a Kyiv concert reads 19:00 everywhere);
 *  without a zone, the visitor's local time. */
function parts(iso: string | null | undefined, tz?: string | null): Parts | null {
  if (!iso) return null;
  const x = new Date(iso);
  if (Number.isNaN(x.getTime())) return null;
  if (tz) {
    try {
      const f = new Intl.DateTimeFormat("en-US", { timeZone: tz, year: "numeric", month: "numeric", day: "numeric", hour: "numeric", minute: "numeric", weekday: "short", hourCycle: "h23" });
      const p = Object.fromEntries(f.formatToParts(x).map((q) => [q.type, q.value]));
      const dows = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
      return { y: +p.year, m: +p.month - 1, day: +p.day, hh: +p.hour % 24, mi: +p.minute, dow: dows.indexOf(p.weekday) };
    } catch { /* unknown zone: fall through to local */ }
  }
  return { y: x.getFullYear(), m: x.getMonth(), day: x.getDate(), hh: x.getHours(), mi: x.getMinutes(), dow: x.getDay() };
}

/** { day: "17", mon: "лис" } for the date badge on cards. */
export function dateBadge(iso: string | null | undefined, lang: Lang, tz?: string | null): { day: string; mon: string } | null {
  const x = parts(iso, tz);
  return x ? { day: String(x.day), mon: MONTHS_SHORT[lang][x.m] } : null;
}

export function timeOf(iso: string | null | undefined, tz?: string | null): string {
  const x = parts(iso, tz);
  return x ? `${String(x.hh).padStart(2, "0")}:${String(x.mi).padStart(2, "0")}` : "";
}

/** "пт, 17 лис · 19:00" */
export function shortWhen(iso: string | null | undefined, lang: Lang, tz?: string | null): string {
  const x = parts(iso, tz);
  if (!x) return "";
  return `${WEEKDAYS_SHORT[lang][x.dow]}, ${x.day} ${MONTHS_SHORT[lang][x.m]} · ${timeOf(iso, tz)}`;
}

/** "пʼятниця, 17 листопада" */
export function longDate(iso: string | null | undefined, lang: Lang, tz?: string | null): string {
  const x = parts(iso, tz);
  if (!x) return "";
  return lang === "en"
    ? `${WEEKDAYS[lang][x.dow]}, ${MONTHS_LONG_GEN[lang][x.m]} ${x.day}`
    : `${WEEKDAYS[lang][x.dow]}, ${x.day} ${MONTHS_LONG_GEN[lang][x.m]}`;
}

export type When = "all" | "today" | "tomorrow" | "weekend" | "week";

/** Does an event start inside the chosen window (local time)? */
export function inWindow(iso: string | null | undefined, w: When, now = new Date(), tz?: string | null): boolean {
  if (w === "all") return true;
  // Calendar days in the event's zone, so the filter agrees with the date printed on the card.
  const x = parts(iso, tz);
  const n = parts(now.toISOString(), tz);
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

/** A short label for a zone in the form: "київський час", else the IANA city part. */
export function tzLabel(tz: string): string {
  const LABELS: Record<string, string> = { "Europe/Kyiv": "київський час", "Europe/Warsaw": "варшавський час", "Asia/Tbilisi": "тбіліський час" };
  return LABELS[tz] ?? (tz.split("/").pop() ?? tz).replace("_", " ");
}

export function initials(name: string | null | undefined): string {
  const parts = (name ?? "").trim().split(/\s+/).filter(Boolean);
  if (parts.length === 0) return "U";
  return (parts[0][0] + (parts[1]?.[0] ?? "")).toUpperCase();
}

/** Ukrainian plural: plural(5, ["місце", "місця", "місць"]). */
export function plural(n: number, forms: [string, string, string]): string {
  const a = Math.abs(n) % 100;
  const b = a % 10;
  if (a > 10 && a < 20) return forms[2];
  if (b > 1 && b < 5) return forms[1];
  if (b === 1) return forms[0];
  return forms[2];
}

export function placesLabel(n: number, lang: Lang): string {
  return lang === "en" ? `${n} ${n === 1 ? "place" : "places"}` : `${n} ${plural(n, ["місце", "місця", "місць"])}`;
}
