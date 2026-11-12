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

function d(iso: string | null | undefined): Date | null {
  if (!iso) return null;
  const x = new Date(iso);
  return Number.isNaN(x.getTime()) ? null : x;
}

/** { day: "17", mon: "лис" } for the date badge on cards. */
export function dateBadge(iso: string | null | undefined, lang: Lang): { day: string; mon: string } | null {
  const x = d(iso);
  return x ? { day: String(x.getDate()), mon: MONTHS_SHORT[lang][x.getMonth()] } : null;
}

export function timeOf(iso: string | null | undefined): string {
  const x = d(iso);
  return x ? `${String(x.getHours()).padStart(2, "0")}:${String(x.getMinutes()).padStart(2, "0")}` : "";
}

/** "пт, 17 лис · 19:00" */
export function shortWhen(iso: string | null | undefined, lang: Lang): string {
  const x = d(iso);
  if (!x) return "";
  return `${WEEKDAYS_SHORT[lang][x.getDay()]}, ${x.getDate()} ${MONTHS_SHORT[lang][x.getMonth()]} · ${timeOf(iso)}`;
}

/** "пʼятниця, 17 листопада" */
export function longDate(iso: string | null | undefined, lang: Lang): string {
  const x = d(iso);
  if (!x) return "";
  return lang === "en"
    ? `${WEEKDAYS[lang][x.getDay()]}, ${MONTHS_LONG_GEN[lang][x.getMonth()]} ${x.getDate()}`
    : `${WEEKDAYS[lang][x.getDay()]}, ${x.getDate()} ${MONTHS_LONG_GEN[lang][x.getMonth()]}`;
}

export type When = "all" | "today" | "tomorrow" | "weekend" | "week";

/** Does an event start inside the chosen window (local time)? */
export function inWindow(iso: string | null | undefined, w: When, now = new Date()): boolean {
  if (w === "all") return true;
  const x = d(iso);
  if (!x) return false;
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const day = 86_400_000;
  const dayIdx = Math.floor((new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime() - start.getTime()) / day);
  if (w === "today") return dayIdx === 0;
  if (w === "tomorrow") return dayIdx === 1;
  if (w === "week") return dayIdx >= 0 && dayIdx < 7;
  // weekend: the coming Saturday and Sunday (today included if it is one)
  const dow = now.getDay();
  const toSat = dow === 0 ? -1 : 6 - dow;
  return dayIdx === toSat || dayIdx === toSat + 1;
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
