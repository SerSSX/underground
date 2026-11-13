// Category key → [uk, en] label (TASK-303 L1: other languages come from web/locales/<lang>.json, keyed by
// the Ukrainian label). Keys are lower-case; events store either case ("Rock" / "rock").
// Plain data in a .mjs so scripts/i18n-extract.mjs can read it without a TypeScript build.

/** @type {Record<string, [string, string]>} */
export const CATEGORY_LABELS = {
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
