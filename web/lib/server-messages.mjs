// Texts the API sends (TASK-303 L1). The API answers in Ukrainian (api/src/*.rs, as of 2026-11-13); the web
// shows them through the same catalogs as its own strings: uk as sent, en from this table, other languages
// from web/locales/<lang>.json keyed by the Ukrainian text. An unknown text falls back to the server's own.
// Plain data in a .mjs so scripts/i18n-extract.mjs puts these keys into locales/_source.json.
// Types: lib/server-messages.d.mts.

import { tr, tf } from "./i18n-core.mjs";

/** Ukrainian server text → English. */
export const SERVER_MESSAGES = {
  // Generic (lib/api.ts check() fallbacks and normalised English codes)
  "Потрібен вхід": "Please sign in",
  "Недостатньо прав": "You don't have access to this",
  "Невідома помилка": "Unknown error",
  "Не знайдено": "Not found",
  "Помилка сервера — спробуйте пізніше": "Server error — please try again later",
  "Немає зʼєднання з сервером": "Can't reach the server",
  "забагато спроб, спробуйте пізніше": "too many attempts, try again later",
  "Цей email уже зареєстровано": "This email is already registered",
  "Некоректний email": "Invalid email",
  "Пароль має містити щонайменше 8 символів": "The password must be at least 8 characters",
  "Введіть пароль": "Enter a password",
  "Введіть імʼя": "Enter a name",
  "Імʼя задовге": "The name is too long",
  "Файл має бути не більше 5 МБ": "The file must be at most 5 MB",
  "Дозволено лише зображення JPEG, PNG або WebP": "Only JPEG, PNG or WebP images are allowed",
  "Порожній файл": "The file is empty",
  "Введіть текст": "Enter some text",
  "Не можна підписатися на себе": "You can't follow yourself",
  "Не можна написати самому собі": "You can't message yourself",
  "Оцінка має бути від 1 до 5": "The rating must be from 1 to 5",
  // Discover section titles (api/src/discover.rs)
  "Для тебе": "For you",
  "Незабаром": "Coming up",
  "Цими вихідними": "This weekend",
  "Популярне": "Popular",
  // Booking, tickets, gifts (api/src/tickets.rs, orders.rs)
  "подія вже завершилась": "the event is over",
  "продаж квитків призупинено організатором": "the organiser has paused bookings",
  "квитки закінчились": "no places left",
  "ця подія має вікове обмеження": "this event has an age limit",
  "платні квитки — через оплату (скоро)": "paid tickets go through checkout (coming soon)",
  "недостатньо вільних місць": "not enough free places",
  "quantity має бути цілим числом від 1 до 5": "quantity must be a whole number from 1 to 5",
  "некоректний JSON": "invalid JSON",
  "отримувач уже має максимум місць на цю подію (5)": "the recipient already has the maximum number of places for this event (5)",
  "квиток неактивний": "the ticket is not active",
  "квиток уже використаний або повернений": "the ticket has already been used or cancelled",
  "термін повернення квитка минув": "it's too late to cancel this ticket",
  "не вдалося передати квиток на цей email — надішліть його подарунком": "couldn't transfer the ticket to this email — send it as a gift instead",
  "не можна подарувати квиток собі": "you can't gift a ticket to yourself",
  "отримувач не відповідає віковому обмеженню події": "the recipient doesn't meet the event's age limit",
  "квиток під спором — передавання недоступне": "the ticket is under dispute — it can't be transferred",
  "квиток уже надіслано в подарунок": "the ticket has already been sent as a gift",
  "email отримувача обов'язковий": "the recipient's email is required",
  "немає активного подарунка для скасування": "there is no pending gift to cancel",
  "термін дії подарунка минув": "the gift has expired",
  "ви не відповідаєте віковому обмеженню події": "you don't meet the event's age limit",
  "промокод недійсний": "invalid promo code",
  "промокод вичерпано": "the promo code has been used up",
  "безкоштовна подія — отримайте квиток без оплати": "free event — get your ticket without paying",
  "замовлення вже оброблене": "the order has already been processed",
  // Events, staff, account (api/src/events.rs, staff.rs, auth.rs, gdpr.rs, passwords.rs, reviews.rs, promos.rs)
  "посилання мають починатися з https:// (або http://)": "links must start with https:// (or http://)",
  "час завершення має бути пізніше за час початку": "the end time must be after the start time",
  "статус цієї події змінити не можна": "this event's status can't be changed",
  "подію скасовано — запрошення неможливе": "the event is cancelled — invitations are closed",
  "термін дії запрошення минув": "the invitation has expired",
  "організатор не може прийняти власне запрошення": "the organiser can't accept their own invitation",
  "дата народження не може бути в майбутньому": "the date of birth can't be in the future",
  "увійдіть ще раз, щоб видалити акаунт": "sign in again to delete your account",
  "посилання недійсне або протерміноване": "the link is invalid or has expired",
  "акаунт недоступний": "the account is unavailable",
  "відгуки відкриються після початку події": "reviews open once the event has started",
  "такий код уже існує": "this code already exists",
  // TASK-304: passwords and pictures (api/src/passwords.rs, uploads.rs)
  "неправильний поточний пароль": "the current password is wrong",
  "введіть поточний пароль": "enter your current password",
  "увійдіть ще раз, щоб задати пароль": "sign in again to set a password",
  "пароль задовгий (до 256 символів)": "the password is too long (up to 256 characters)",
  "файл не є зображенням JPEG, PNG або WebP": "the file is not a JPEG, PNG or WebP image",
  "зображення завелике: до 32 мегапікселів": "the image is too large: up to 32 megapixels",
  "пошта тимчасово недоступна, спробуйте пізніше": "mail is temporarily unavailable, try again later",
  "зображення надто складне для обробки — збережіть як JPEG або зменште розмір": "the image is too complex to process — save it as JPEG or make it smaller",
};

/** Server texts with numbers in them: the regex captures the values, in the order of `vars`. */
export const SERVER_PATTERNS = [
  { re: /^Помилка (\d+)$/, uk: "Помилка {status}", en: "Error {status}", vars: ["status"] },
  {
    re: /^на одну людину — не більше (\d+) місць \(у вас уже (\d+)\)$/,
    uk: "на одну людину — не більше {max} місць (у вас уже {held})",
    en: "at most {max} places per person (you already have {held})",
    vars: ["max", "held"],
  },
  {
    re: /^забагато активних запрошень \(до (\d+)\); скасуйте непотрібні$/,
    uk: "забагато активних запрошень (до {max}); скасуйте непотрібні",
    en: "too many pending invitations (up to {max}); cancel the ones you don't need",
    vars: ["max"],
  },
];

/** English codes some API errors (and fetch itself) use → the Ukrainian source text above. `null` means:
 *  no text of its own, use the HTTP status fallback ("Потрібен вхід"). Keys are lower-case. */
export const SERVER_CODES = {
  unauthorized: null,
  forbidden: "Недостатньо прав",
  "not found": "Не знайдено",
  "internal error": "Помилка сервера — спробуйте пізніше",
  "database error": "Помилка сервера — спробуйте пізніше",
  "email taken": "Цей email уже зареєстровано",
  "email already registered": "Цей email уже зареєстровано",
  "invalid email": "Некоректний email",
  "bad email": "Некоректний email",
  "password must be at least 8 characters": "Пароль має містити щонайменше 8 символів",
  "password is required": "Введіть пароль",
  "name is required": "Введіть імʼя",
  "name must not be blank": "Введіть імʼя",
  "name is too long": "Імʼя задовге",
  "file must be at most 5 mb": "Файл має бути не більше 5 МБ",
  "only jpeg/png/webp images are allowed": "Дозволено лише зображення JPEG, PNG або WebP",
  "empty file": "Порожній файл",
  "text is required": "Введіть текст",
  "cannot follow yourself": "Не можна підписатися на себе",
  "cannot message yourself": "Не можна написати самому собі",
  "rating must be between 1 and 5": "Оцінка має бути від 1 до 5",
  "invalid json": "некоректний JSON",
  // fetch() network failures: Chrome, Safari, Firefox
  "failed to fetch": "Немає зʼєднання з сервером",
  "load failed": "Немає зʼєднання з сервером",
  "networkerror when attempting to fetch resource.": "Немає зʼєднання з сервером",
};

/** The Ukrainian source for a raw server text: English codes mapped, anything else unchanged.
 *  Returns null for "no usable text" (empty, or a code that means: use the status fallback). */
export function canonicalServerText(raw) {
  if (typeof raw !== "string" || raw.trim() === "") return null;
  const k = raw.trim().toLowerCase();
  if (Object.prototype.hasOwnProperty.call(SERVER_CODES, k)) return SERVER_CODES[k];
  return raw;
}

/** A server text in the reader's language: known texts translate, unknown ones stay as the server wrote them. */
export function serverText(lang, raw) {
  const text = canonicalServerText(raw);
  if (text === null) return typeof raw === "string" ? raw : "";
  if (lang === "uk") return text;
  // i18n-dynamic: the keys reach locales/_source.json from SERVER_MESSAGES and SERVER_PATTERNS above.
  if (Object.prototype.hasOwnProperty.call(SERVER_MESSAGES, text)) return tr(lang, text, SERVER_MESSAGES[text]);
  for (const p of SERVER_PATTERNS) {
    const m = p.re.exec(text);
    if (m) return tf(lang, p.uk, p.en, Object.fromEntries(p.vars.map((v, i) => [v, m[i + 1]]))); // i18n-dynamic
  }
  return text;
}
