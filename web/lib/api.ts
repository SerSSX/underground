import { safeNextFor } from "./safe-next.mjs";
import { currentLang } from "./i18n-core.mjs";
import { canonicalServerText, serverText } from "./server-messages.mjs";
import { mapsHref } from "./geo-core.mjs";
const BASE = process.env.NEXT_PUBLIC_API_BASE_URL || "http://localhost:8080";
const TOKEN_KEY = "ug_web_token";

/**
 * API base for server-side (SSR) fetches. Node's fetch (undici) resolves `localhost` to IPv6
 * `::1` first; a dev API bound to IPv4 `0.0.0.0` then refuses the connection, so SSR data comes
 * back empty. Use `127.0.0.1` server-side. No-op for real production domains.
 */
export function serverApiBase(): string {
  // Rewrite only a whole-host `localhost` (followed by port/path/end), never a substring like
  // `api.mylocalhost.com`. No-op for real production domains.
  return BASE.replace(/\/\/localhost(?=[:/]|$)/, "//127.0.0.1");
}

export function getToken(): string | null {
  if (typeof window === "undefined") return null;
  return window.localStorage.getItem(TOKEN_KEY);
}
export function setToken(token: string): void {
  window.localStorage.setItem(TOKEN_KEY, token);
}
export function clearToken(): void {
  window.localStorage.removeItem(TOKEN_KEY);
}
function authHeaders(): Record<string, string> {
  const token = getToken();
  return token ? { authorization: `Bearer ${token}` } : {};
}

/** An API error with its HTTP status, so callers decide by status (401 = signed out) and show the server's
 *  own text (e.g. "увійдіть ще раз, щоб видалити акаунт") instead of a generic one. */
export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}
async function check(res: Response): Promise<Response> {
  if (res.ok) return res;
  const body = (await res.json().catch(() => null)) as { error?: string } | null;
  const fallback = res.status === 401 ? "Потрібен вхід" : res.status === 403 ? "Недостатньо прав" : `Помилка ${res.status}`;
  // The message keeps the Ukrainian source text: the API's English codes ("unauthorized", "email taken") are
  // mapped to it (lib/server-messages.mjs), its Ukrainian messages come through as written. errorMessage()
  // shows it in the reader's language.
  throw new ApiError(res.status, canonicalServerText(body?.error) ?? fallback);
}
export async function apiGet<T>(path: string, signal?: AbortSignal): Promise<T> {
  return (await (await check(await fetch(`${BASE}${path}`, { headers: authHeaders(), signal }))).json()) as T;
}
export async function apiSend<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await check(
    await fetch(`${BASE}${path}`, {
      method,
      headers: { ...authHeaders(), "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    }),
  );
  const text = await res.text();
  return (text ? JSON.parse(text) : null) as T;
}
export async function uploadImage(file: File): Promise<string> {
  const form = new FormData();
  form.append("file", file);
  const res = await check(await fetch(`${BASE}/api/uploads`, { method: "POST", headers: authHeaders(), body: form }));
  return ((await res.json()) as { url: string }).url;
}
export function isUnauthorized(e: unknown): boolean {
  return e instanceof ApiError && e.status === 401;
}
/** An error for the screen, in the active language: known server texts translate, unknown ones are shown as
 *  the server wrote them (TASK-303 L1). */
export function errorMessage(e: unknown): string {
  return serverText(currentLang(), e instanceof Error && e.message ? e.message : "Невідома помилка");
}

/**
 * Sanitize a `?next=` redirect target so it can only point back into this app.
 * Rejects absolute URLs (`https://evil.com`), protocol-relative (`//evil.com`),
 * and anything not starting with a single `/` — preventing open-redirect abuse.
 */
/** A same-site path to go to after sign-in, or `fallback` (logic and tests in lib/safe-next.mjs). */
export function safeNext(next: string | null | undefined, fallback = "/"): string {
  if (typeof window === "undefined") return fallback;
  return safeNextFor(next, window.location.origin, fallback);
}

/** Best-effort product telemetry (TASK-017 M4). Never blocks or throws into the UI. */
export async function logTelemetry(name: string, props: Record<string, unknown> = {}): Promise<void> {
  try {
    await fetch(`${BASE}/api/telemetry`, {
      method: "POST",
      headers: { ...authHeaders(), "content-type": "application/json" },
      body: JSON.stringify({ name, props }),
      keepalive: true,
    });
  } catch {
    /* swallow: telemetry must not affect UX */
  }
}

// ---- types ----
export interface UserDto {
  id: string;
  email: string;
  name: string;
  role: string;
  avatar_url: string | null;
  bio: string | null;
  is_organizer: boolean;
  dob?: string | null;
}

/** Minimum-age ratings an organizer can set (empty = unrestricted). */
export const AGE_RATINGS = ["", "13", "16", "18", "21"];
export interface EventDto {
  id: string;
  organizer_id: string | null;
  title: string;
  description: string | null;
  poster_url: string | null;
  category: string;
  city: string | null;
  venue_address: string | null;
  starts_at: string | null;
  ends_at?: string | null;
  price: string;
  currency: string;
  tickets_left: number;
  /** IANA zone the event's local time is shown in (e.g. Europe/Kyiv). */
  timezone?: string;
  /** Capacity the organiser set (0 when an older API omits it). */
  tickets_total?: number;
  status: string;
  cost_type: string;
  video_url: string | null;
  gallery_urls: string[];
  links: string[];
  sales_closed: boolean;
  age_rating?: string | null;
  country?: string | null;
  /** ISO 3166-1 alpha-2 of the event's place (TASK-303 G1). */
  country_code?: string | null;
  lat?: number | null;
  lng?: number | null;
  // Discovery-only (present on /api/discover responses).
  reason?: string | null;
  saved_by_me?: boolean;
  saves_count?: number;
  distance_km?: number | null;
}

export interface CityDto {
  name: string;
  country: string;
  lat: number;
  lng: number;
  country_code?: string | null;
  /** IANA zone (TASK-303 G1); null on rows the API cannot place. */
  timezone?: string | null;
  /** "seed" (ours) or "osm" (learned from the geocoder; answers only its own country — G1 fix batch). */
  source?: string | null;
}
/** Our city gazetteer by name prefix (not the geocoder): what the API itself knows about a city. */
export async function searchCities(q: string, signal?: AbortSignal): Promise<CityDto[]> {
  return apiGet<CityDto[]>(`/api/cities?q=${encodeURIComponent(q)}`, signal);
}

export interface UserBrief {
  id: string;
  name: string;
  avatar_url: string | null;
}
export interface EventSocial {
  interested_count: number;
  going_count: number;
  friends_interested: UserBrief[];
  friends_going: UserBrief[];
}
export async function eventSocial(id: string): Promise<EventSocial> {
  return apiGet<EventSocial>(`/api/events/${id}/social`);
}
export async function similarEvents(id: string): Promise<EventDto[]> {
  return apiGet<EventDto[]>(`/api/events/${id}/similar`);
}
export interface TicketDto {
  id: string;
  event_id: string;
  event_title: string;
  event_venue_address?: string | null;
  event_poster_url?: string | null;
  event_timezone?: string | null;
  event_ends_at?: string | null;
  event_city: string | null;
  event_starts_at: string | null;
  qr_token: string;
  status: string;
  /** Paid order behind this ticket (null for free reservations) — enables "dispute". */
  order_id: string | null;
  /** held|clearing → dispute possible; frozen → already disputed; else not disputable. */
  escrow_status: string | null;
  /** Amount paid + purchase time (for the receipt). price is "0" for free tickets. */
  price: string;
  created_at: string;
}
export interface ReviewDto {
  id: string;
  author_name: string;
  rating: number;
  text: string | null;
  created_at: string;
}
export interface EventReviews {
  avg_rating: number | null;
  count: number;
  reviews: ReviewDto[];
}
export interface CheckoutResp {
  order_id: string;
  mode: string;
  amount: string;
  currency: string;
  test_mode: boolean;
}

// ---- auth ----
export async function login(email: string, password: string): Promise<string> {
  const data = await apiSend<{ token: string }>("POST", "/api/auth/login", { email, password });
  return data.token;
}
/** Sign-up carries the UI language (one of the 10) so the first emails and notifications are in it
 *  (TASK-303 G2 fix F1; an API without the field ignores it). */
export async function register(email: string, password: string, name: string): Promise<string> {
  const data = await apiSend<{ token: string }>("POST", "/api/auth/register", { email, password, name, locale: currentLang() });
  return data.token;
}
export async function forgotPassword(email: string): Promise<void> {
  await apiSend("POST", "/api/auth/forgot-password", { email });
}
export async function resetPassword(token: string, password: string): Promise<void> {
  await apiSend("POST", "/api/auth/reset-password", { token, password });
}
// Passwordless email one-time code (TASK-115). request → email a code; verify → app-JWT.
export async function requestEmailCode(email: string): Promise<void> {
  await apiSend("POST", "/api/auth/email/request-code", { email });
}
export async function verifyEmailCode(email: string, code: string): Promise<string> {
  // May create the account: it starts in the UI language, as with register().
  const data = await apiSend<{ token: string }>("POST", "/api/auth/email/verify-code", { email, code, locale: currentLang() });
  return data.token;
}
export async function getMe(): Promise<UserDto> {
  return apiGet<UserDto>("/api/me");
}

// ---- events (public) ----
export interface EventFilters {
  q?: string;
  city?: string;
  category?: string;
  free?: boolean;
  sort?: string;
}
export async function getEvents(f: EventFilters = {}): Promise<EventDto[]> {
  const p = new URLSearchParams();
  if (f.q) p.set("q", f.q);
  if (f.city) p.set("city", f.city);
  if (f.category) p.set("category", f.category);
  if (f.free) p.set("free", "true");
  if (f.sort) p.set("sort", f.sort);
  const qs = p.toString();
  return apiGet<EventDto[]>(`/api/events${qs ? `?${qs}` : ""}`);
}
export async function getEvent(id: string): Promise<EventDto> {
  return apiGet<EventDto>(`/api/events/${id}`);
}
export async function discoverEvents(): Promise<EventDto[]> {
  return apiGet<EventDto[]>("/api/discover");
}
export interface DiscoverSection {
  key: string;
  title: string;
  events: EventDto[];
}
/** Time-aware sectioned Афіша (BR-024): soon / weekend / for_you / popular. */
export async function discoverSections(): Promise<DiscoverSection[]> {
  return apiGet<DiscoverSection[]>("/api/discover/sections");
}
export async function mySaves(): Promise<EventDto[]> {
  return apiGet<EventDto[]>("/api/me/saves");
}

// ---- notifications inbox (parity with Android «Вхідні») ----
export interface NotificationDto {
  id: string;
  kind: string;
  title: string;
  body: string | null;
  event_id: string | null;
  read: boolean;
  created_at: string;
}
export async function myNotifications(): Promise<NotificationDto[]> {
  return apiGet<NotificationDto[]>("/api/me/notifications");
}
export async function markNotificationsRead(): Promise<void> {
  await apiSend("POST", "/api/me/notifications/read", undefined);
}
export async function saveEvent(id: string): Promise<void> {
  await apiSend("POST", `/api/events/${id}/save`);
}
export async function unsaveEvent(id: string): Promise<void> {
  await apiSend("DELETE", `/api/events/${id}/save`);
}
export async function getEventReviews(id: string): Promise<EventReviews> {
  return apiGet<EventReviews>(`/api/events/${id}/reviews`);
}
export async function postReview(eventId: string, rating: number, text: string): Promise<void> {
  await apiSend("POST", "/api/reviews", { event_id: eventId, rating, text: text || null });
}

// ---- tickets ----
export async function myTickets(): Promise<TicketDto[]> {
  return apiGet<TicketDto[]>("/api/me/tickets");
}
export async function reserveTicket(eventId: string): Promise<TicketDto> {
  return apiSend<TicketDto>("POST", `/api/events/${eventId}/reserve`, undefined);
}
/** Book `quantity` places (1–5) at once; each place is its own ticket with its own QR. Returns the first. */
export async function reserveTickets(eventId: string, quantity: number): Promise<TicketDto> {
  return apiSend<TicketDto>("POST", `/api/events/${eventId}/reserve`, { quantity });
}
export async function checkout(eventId: string): Promise<CheckoutResp> {
  return apiSend<CheckoutResp>("POST", `/api/events/${eventId}/checkout`, undefined);
}
export async function payTest(orderId: string): Promise<TicketDto> {
  return apiSend<TicketDto>("POST", `/api/orders/${orderId}/pay-test`, undefined);
}
export async function returnTicket(ticketId: string): Promise<TicketDto> {
  return apiSend<TicketDto>("POST", `/api/tickets/${ticketId}/return`, undefined);
}
export async function transferTicket(ticketId: string, email: string): Promise<TicketDto> {
  return apiSend<TicketDto>("POST", `/api/tickets/${ticketId}/transfer`, { email });
}
export async function disputeOrder(orderId: string): Promise<void> {
  await apiSend("POST", `/api/orders/${orderId}/dispute`, undefined);
}

// ---- gift / claim (TASK-110/114, WF-GiftClaim) ----
export interface GiftPreviewDto {
  event_timezone?: string | null;
  from_name: string;
  event_title: string;
  event_starts_at: string | null;
  event_city: string | null;
  event_poster_url: string | null;
  expires_at: string;
}
export interface GiftResp {
  claim_token: string;
  to_email: string;
  expires_at: string;
}
/** Public preview of a gift by its claim token (no auth) — powers the claim landing. */
export async function giftPreview(token: string): Promise<GiftPreviewDto> {
  return apiGet<GiftPreviewDto>(`/api/gifts/${encodeURIComponent(token)}`);
}
export async function giftTicket(ticketId: string, email: string): Promise<GiftResp> {
  return apiSend<GiftResp>("POST", `/api/tickets/${ticketId}/gift`, { email });
}
/** Owner cancels an outstanding (unclaimed) gift, reclaiming the ticket → active. */
export async function revokeGift(ticketId: string): Promise<TicketDto> {
  return apiSend<TicketDto>("DELETE", `/api/tickets/${ticketId}/gift`, undefined);
}
export interface QrResp {
  token: string;
  expires_at: string;
}
/** Mint a signed, short-TTL door-QR for the owner's active ticket (TASK-118-B, BR-023). */
export async function ticketQr(ticketId: string): Promise<QrResp> {
  return apiGet<QrResp>(`/api/tickets/${ticketId}/qr`);
}
export async function claimTicket(token: string): Promise<TicketDto> {
  return apiSend<TicketDto>("POST", "/api/tickets/claim", { token });
}

// ---- prefs / onboarding (UX-R) ----
export interface Prefs {
  categories: string[];
  city: string | null;
  country?: string | null;
  /** ISO code of the prefs city (TASK-303 G1 fix batch). */
  country_code?: string | null;
  /** One of the 10 UI languages. */
  locale?: string | null;
  /** An explicit column: a sign-up with `locale` creates a row with onboarded = false. */
  onboarded: boolean;
}
export async function getPrefs(): Promise<Prefs> {
  return apiGet<Prefs>("/api/me/prefs");
}
/** Save interests and city. Sends the UI language as is (the API stores all 10 since the G1 fix batch) and the
 *  city's country code when known, so a learned city resolves in the right country. */
export async function putPrefs(categories: string[], city: string | null, countryCode?: string | null): Promise<Prefs> {
  const country_code = city && countryCode && /^[A-Za-z]{2}$/.test(countryCode) ? countryCode.toUpperCase() : null;
  return apiSend<Prefs>("PUT", "/api/me/prefs", { categories, city, country_code, locale: currentLang() });
}

// ---- organizer cabinet ----
export interface OrganizerStats {
  events_count: number;
  tickets_sold: number;
  tickets_validated: number;
  revenue: string;
  escrow_held: string;
  escrow_cleared: string;
  paid_out: string;
}
export interface EventTicket {
  id: string;
  owner_name: string;
  owner_email: string;
  status: string;
  price: string;
  created_at: string;
  validated_at: string | null;
}
export interface EventInput {
  title: string;
  description?: string | null;
  category: string;
  city?: string | null;
  venue_address?: string | null;
  starts_at?: string | null;
  ends_at?: string | null;
  price: string;
  tickets_total: number;
  cost_type: string;
  poster_url?: string | null;
  video_url?: string | null;
  links?: string[];
  /** `published` (default) or `draft` (TASK-017 M8). */
  status?: string;
  /** Minimum-age rating: 13|16|18|21 or "" for unrestricted (P1 #77). */
  age_rating?: string | null;
  /** IANA zone the times are local to (default on the API: Europe/Kyiv). */
  timezone?: string | null;
  /** The chosen address's point and country (TASK-303 G2): both coordinates or neither. */
  lat?: number | null;
  lng?: number | null;
  country?: string | null;
  country_code?: string | null;
  /** Update only: true removes the end time (sent when the organiser empties the field). */
  clear_ends_at?: boolean;
}

// ---- promo codes (organizer, TASK-017 M8) ----
export interface PromoCode {
  id: string;
  event_id: string;
  code: string;
  percent_off: number;
  max_uses: number | null;
  used_count: number;
  expires_at: string | null;
  active: boolean;
  created_at: string;
}
export async function listPromos(eventId: string): Promise<PromoCode[]> {
  return apiGet<PromoCode[]>(`/api/events/${eventId}/promos`);
}
export async function createPromo(eventId: string, code: string, percentOff: number): Promise<PromoCode> {
  return apiSend<PromoCode>("POST", `/api/events/${eventId}/promos`, { code, percent_off: percentOff });
}
export async function deletePromo(id: string): Promise<void> {
  await apiSend("DELETE", `/api/promos/${id}`);
}
export interface DailySales {
  day: string;
  tickets: number;
  revenue: string;
}
export async function organizerAnalytics(): Promise<DailySales[]> {
  return apiGet<DailySales[]>("/api/organizer/analytics");
}

export interface MyPayout {
  id: string;
  amount: string;
  currency: string;
  order_count: number;
  status: string;
  failure_reason: string | null;
  created_at: string;
}
export async function organizerPayouts(): Promise<MyPayout[]> {
  return apiGet<MyPayout[]>("/api/organizer/payouts");
}

export async function myEvents(): Promise<EventDto[]> {
  return apiGet<EventDto[]>("/api/me/events");
}
export async function organizerStats(): Promise<OrganizerStats> {
  return apiGet<OrganizerStats>("/api/organizer/stats");
}
export async function eventTickets(id: string): Promise<EventTicket[]> {
  return apiGet<EventTicket[]>(`/api/events/${id}/tickets`);
}
export async function createEvent(body: EventInput): Promise<EventDto> {
  return apiSend<EventDto>("POST", "/api/events", body);
}
export async function updateEvent(
  id: string,
  body: Partial<EventInput> & { status?: string; sales_closed?: boolean },
): Promise<EventDto> {
  return apiSend<EventDto>("PUT", `/api/events/${id}`, body);
}
export async function cancelEvent(id: string): Promise<void> {
  await check(await fetch(`${BASE}/api/events/${id}`, { method: "DELETE", headers: authHeaders() }));
}

// ---- helpers ----
/** A datetime-local value ("2026-12-12T19:30", the organiser's own clock) → a real UTC instant.
 *  (It used to append "Z", storing wall-clock time as if it were UTC; pages that converted to local time then
 *  showed it shifted by the time-zone offset.) */
export function toIso(local: string, tz?: string | null): string | null {
  if (!local) return null;
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(local);
  if (!m) return null;
  const wall = Date.UTC(+m[1], +m[2] - 1, +m[3], +m[4], +m[5]);
  if (!tz) {
    const d = new Date(local);
    return Number.isNaN(d.getTime()) ? null : d.toISOString();
  }
  // Wall-clock time in `tz` → UTC: subtract the zone's offset at that moment (twice, to settle DST edges).
  let t = wall - tzOffsetMs(wall, tz);
  t = wall - tzOffsetMs(t, tz);
  return new Date(t).toISOString();
}

/** Offset of an IANA zone from UTC at an instant, in ms (Kyiv in winter → +2 h). */
export function tzOffsetMs(at: number, tz: string): number {
  try {
    const f = new Intl.DateTimeFormat("en-US", { timeZone: tz, year: "numeric", month: "numeric", day: "numeric", hour: "numeric", minute: "numeric", second: "numeric", hourCycle: "h23" });
    const p = Object.fromEntries(f.formatToParts(new Date(at)).map((q) => [q.type, q.value]));
    return Date.UTC(+p.year, +p.month - 1, +p.day, +p.hour % 24, +p.minute, +p.second) - Math.floor(at / 1000) * 1000;
  } catch {
    return -new Date(at).getTimezoneOffset() * 60_000;
  }
}

/** The visitor's own IANA zone (sent with a new event so its local time is the organiser's clock). */
export function browserTz(): string | null {
  try { return Intl.DateTimeFormat().resolvedOptions().timeZone || null; } catch { return null; }
}
/** Google Maps search for an address and city (no point known). With a point: lib/geo-core.mjs mapsHref. */
export function mapsUrl(address: string | null, city: string | null): string | null {
  return mapsHref({ venue_address: address, city }, null);
}
// Display symbol per ISO currency (mirrors the backend currency map). Falls back to the code.
const CURRENCY_SYMBOLS: Record<string, string> = {
  UAH: "₴", USD: "$", EUR: "€", GBP: "£", PLN: "zł", CZK: "Kč", HUF: "Ft", CHF: "CHF",
  SEK: "kr", DKK: "kr", NOK: "kr", TRY: "₺", RON: "lei", BGN: "лв", CAD: "C$", MXN: "$",
  BRL: "R$", ARS: "$", JPY: "¥", CNY: "¥", KRW: "₩", SGD: "S$", THB: "฿", INR: "₹",
  AED: "AED", ILS: "₪", EGP: "E£", ZAR: "R", NGN: "₦", AUD: "A$", NZD: "NZ$",
};
export function currencySymbol(code: string): string {
  return CURRENCY_SYMBOLS[code] ?? code;
}
export function fmtPrice(amount: string, currency: string): string {
  return `${amount.replace(/\.00$/, "")} ${currencySymbol(currency)}`;
}

/** Best-effort: persist the chosen locale to the user's prefs (logged-in only), so
 *  server-generated notifications match the UI language. Preserves existing prefs.
 *  Only for an onboarded user: the PUT creates the prefs row, and a row is what marks onboarding done, so
 *  syncing a brand-new account would skip its interests step (TASK-303 G2 fix F1). New accounts get their
 *  language from the sign-up body instead. */
export async function syncUserLocale(locale: string): Promise<void> {
  if (!getToken()) return;
  try {
    const prefs = await apiGet<Prefs>("/api/me/prefs");
    if (prefs.onboarded === false) return;
    await apiSend("PUT", "/api/me/prefs", { categories: prefs.categories, city: prefs.city, country_code: prefs.country_code ?? null, locale });
  } catch {
    /* ignore: locale sync is non-essential */
  }
}
/** An event is over at its end time, or 12 h after the start when no end is set. Until then a guest can
 *  still show the QR and staff can still scan (a concert that started at 19:00 still admits at 19:05). */
export function isOver(startsAt: string | null | undefined, endsAt?: string | null): boolean {
  const start = startsAt ? Date.parse(startsAt) : NaN;
  let end = endsAt ? Date.parse(endsAt) : NaN;
  // An end at or before the start is bad data: ignore it rather than close the doors early.
  if (!Number.isFinite(end) || (Number.isFinite(start) && end <= start)) end = Number.isFinite(start) ? start + 12 * 3600_000 : NaN;
  return Number.isFinite(end) && end < Date.now();
}
/** Has it started? (Booking closes at the start; the API refuses after that.) */
export function hasStarted(startsAt: string | null | undefined): boolean {
  const t = startsAt ? Date.parse(startsAt) : NaN;
  return Number.isFinite(t) && t <= Date.now();
}

/** Only absolute http(s) URLs become links; anything else (javascript:, data:, relative junk) renders as nothing. */
export function safeHref(u: string | null | undefined): string | null {
  if (!u) return null;
  try {
    const x = new URL(u);
    return x.protocol === "https:" || x.protocol === "http:" ? x.href : null;
  } catch {
    return null;
  }
}

export function isPast(iso: string | null): boolean {
  if (!iso || iso.length < 19) return false;
  return iso.slice(0, 19) < new Date().toISOString().slice(0, 19);
}

// ---- account: profile / age / GDPR (P1 #73, #77) ----
export interface PatchMeBody {
  name?: string;
  bio?: string;
  avatar_url?: string;
  is_organizer?: boolean;
  /** Date of birth YYYY-MM-DD (age-gating). */
  dob?: string;
}
export async function patchMe(body: PatchMeBody): Promise<UserDto> {
  return apiSend<UserDto>("PATCH", "/api/me", body);
}
export async function exportMyData(): Promise<unknown> {
  return apiGet<unknown>("/api/me/export");
}
/** Password accounts confirm with the password; accounts without one (email code, Google) type DELETE. */
export async function deleteMyAccount(proof: { password: string } | { confirm: "DELETE" }): Promise<void> {
  await apiSend("DELETE", "/api/me", proof);
}

// ---- organizer KYC (P1 #78) ----
export interface KycView {
  kyc_status: string;
  has_bank_details: boolean;
}
export async function getKyc(): Promise<KycView> {
  return apiGet<KycView>("/api/me/kyc");
}
export async function submitKyc(name: string, iban: string, swift: string): Promise<void> {
  await apiSend("POST", "/api/me/kyc", { name, iban, swift });
}

// ---- abuse reporting (P1 #76) ----
export type ReportTarget = "event" | "user" | "post" | "review";
export async function createReport(
  targetType: ReportTarget,
  targetId: string,
  reason: string,
  details?: string,
): Promise<void> {
  await apiSend("POST", "/api/reports", {
    target_type: targetType,
    target_id: targetId,
    reason,
    details: details || null,
  });
}
export const EVENT_CATEGORIES = [
  "Concert", "Rock", "Metal", "Electronic", "Hip-Hop", "Jazz", "Pop", "Folk", "Classical",
  "DJ Set", "Festival", "Karaoke", "Stand-Up", "Theatre", "Improv", "Dance", "Poetry", "Cinema",
  "Exhibition", "Photography", "Fashion", "Literature", "Craft", "Lecture", "Conference", "Workshop",
  "Course", "Networking", "Startup", "Hackathon", "Sport", "Yoga", "Hike", "Run", "Cycling", "Extreme",
  "Party", "Meetup", "Charity", "Family", "Food & Drink", "Tasting", "Board Games", "Quiz", "Market",
  "Gaming", "Spiritual", "Other",
];

// ---- public profiles ----
export interface PublicProfile {
  id: string;
  name: string;
  avatar_url: string | null;
  bio: string | null;
  is_organizer: boolean;
  verified: boolean;
  events_count: number;
  rating_avg: number | null;
  rating_count: number;
}
export async function getProfile(id: string): Promise<PublicProfile> {
  return apiGet<PublicProfile>(`/api/users/${id}`);
}

// ---- direct messages ----
export interface ConversationDto {
  peer_id: string;
  peer_name: string;
  peer_avatar: string | null;
  last_text: string;
  last_at: string;
  last_from_me: boolean;
  unread: number;
}
export interface MessageDto {
  id: string;
  sender_id: string;
  recipient_id: string;
  text: string;
  read: boolean;
  created_at: string;
}
export async function myConversations(): Promise<ConversationDto[]> {
  return apiGet<ConversationDto[]>("/api/me/conversations");
}
export async function messageThread(peerId: string): Promise<MessageDto[]> {
  return apiGet<MessageDto[]>(`/api/messages/${peerId}`);
}
export async function sendMessage(recipientId: string, text: string): Promise<MessageDto> {
  return apiSend<MessageDto>("POST", "/api/messages", { recipient_id: recipientId, text });
}

// ---- organiser: guests, door scan, staff (TASK-302) ----
/** One attendee of an event: the organiser sees only the name and the number of places (brief 2026-11-11). */
export interface GuestDto {
  name: string;
  places: number;
  checked_in: number;
  booked_at: string;
}
export async function eventGuests(eventId: string): Promise<GuestDto[]> {
  return apiGet<GuestDto[]>(`/api/events/${eventId}/guests`);
}
export interface ScanResult {
  valid: boolean;
  /** valid | already_used | wrong_event | returned | inactive | not_found */
  reason: string;
  event_title: string;
  holder_name?: string | null;
  places?: number | null;
  checked_in?: number | null;
}
export async function scanTicket(eventId: string, qrToken: string): Promise<ScanResult> {
  return apiSend<ScanResult>("POST", `/api/events/${eventId}/scan`, { qr_token: qrToken });
}
export async function inviteStaff(eventId: string): Promise<{ invite_token: string; expires_at: string }> {
  return apiSend("POST", `/api/events/${eventId}/staff/invite`, {});
}
export async function acceptStaffInvite(token: string): Promise<{ event_id: string }> {
  return apiSend("POST", "/api/staff/accept", { token });
}
export async function myStaffEvents(): Promise<EventDto[]> {
  return apiGet<EventDto[]>("/api/me/staff/events");
}
export interface StaffMember { user_id: string; name: string; added_at?: string | null }
export interface StaffInvite { id: string; status: string; expires_at: string; created_at?: string | null }
export interface StaffList { staff: StaffMember[]; invites: StaffInvite[] }
export async function listStaff(eventId: string): Promise<StaffList> {
  return apiGet<StaffList>(`/api/events/${eventId}/staff`);
}
export async function revokeStaffInvite(eventId: string, inviteId: string): Promise<void> {
  await apiSend("DELETE", `/api/events/${eventId}/staff/invites/${inviteId}`);
}
export async function removeStaff(eventId: string, userId: string): Promise<void> {
  await apiSend("DELETE", `/api/events/${eventId}/staff/${userId}`);
}
