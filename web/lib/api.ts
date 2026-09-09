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

async function check(res: Response): Promise<Response> {
  if (res.status === 401) throw new Error("Потрібен вхід");
  if (res.status === 403) throw new Error("Недостатньо прав");
  if (!res.ok) {
    const body = (await res.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error || `Помилка ${res.status}`);
  }
  return res;
}
export async function apiGet<T>(path: string): Promise<T> {
  return (await (await check(await fetch(`${BASE}${path}`, { headers: authHeaders() }))).json()) as T;
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
export function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : "Невідома помилка";
}

/**
 * Sanitize a `?next=` redirect target so it can only point back into this app.
 * Rejects absolute URLs (`https://evil.com`), protocol-relative (`//evil.com`),
 * and anything not starting with a single `/` — preventing open-redirect abuse.
 */
export function safeNext(next: string | null | undefined, fallback = "/"): string {
  if (!next || !next.startsWith("/") || next.startsWith("//") || next.startsWith("/\\")) {
    return fallback;
  }
  return next;
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
  price: string;
  currency: string;
  tickets_left: number;
  status: string;
  cost_type: string;
  video_url: string | null;
  gallery_urls: string[];
  links: string[];
  sales_closed: boolean;
  age_rating?: string | null;
  country?: string | null;
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
}
export async function searchCities(q: string): Promise<CityDto[]> {
  return apiGet<CityDto[]>(`/api/cities?q=${encodeURIComponent(q)}`);
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
  event_title: string;
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
export async function register(email: string, password: string, name: string): Promise<string> {
  const data = await apiSend<{ token: string }>("POST", "/api/auth/register", { email, password, name });
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
  const data = await apiSend<{ token: string }>("POST", "/api/auth/email/verify-code", { email, code });
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
  onboarded: boolean;
}
export async function getPrefs(): Promise<Prefs> {
  return apiGet<Prefs>("/api/me/prefs");
}
export async function putPrefs(categories: string[], city: string | null): Promise<Prefs> {
  return apiSend<Prefs>("PUT", "/api/me/prefs", { categories, city });
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
export function toIso(local: string): string | null {
  if (!local) return null;
  return local.length === 16 ? `${local}:00Z` : `${local}Z`;
}
export function mapsUrl(address: string | null, city: string | null): string | null {
  const q = [address, city].filter(Boolean).join(", ");
  return q ? `https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(q)}` : null;
}
const MONTHS = ["січ", "лют", "бер", "кві", "тра", "чер", "лип", "сер", "вер", "жов", "лис", "гру"];
export function fmtDate(iso: string | null): string {
  if (!iso || iso.length < 10) return "";
  const m = Number(iso.slice(5, 7));
  const d = Number(iso.slice(8, 10));
  const day = iso.length >= 16 ? ` · ${iso.slice(11, 16)}` : "";
  return `${d} ${MONTHS[m - 1] ?? ""}${day}`;
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
 *  server-generated notifications match the UI language. Preserves existing prefs. */
export async function syncUserLocale(locale: string): Promise<void> {
  if (!getToken()) return;
  try {
    const prefs = await apiGet<{ categories: string[]; city: string | null }>("/api/me/prefs");
    await apiSend("PUT", "/api/me/prefs", { categories: prefs.categories, city: prefs.city, locale });
  } catch {
    /* ignore: locale sync is non-essential */
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
export async function deleteMyAccount(password: string): Promise<void> {
  await apiSend("DELETE", "/api/me", { password });
}

// ---- organiser payouts (PAY-5) ----
// The organiser is the merchant of record for their own tickets, so their identity and bank
// details are entered on the payment provider's own pages and never reach us. What we read back
// is only whether that account can take money yet.
export interface PayoutStatus {
  charges_enabled: boolean;
  payouts_enabled: boolean;
  requirements_due: string[];
  connected: boolean;
  country: string | null;
  kyc_status: string;
}
export async function getPayoutStatus(): Promise<PayoutStatus> {
  return apiGet<PayoutStatus>("/api/me/payouts/status");
}
/// Returns a single-use URL on the provider's site. Send the organiser there; do not store it.
export async function connectPayouts(country: string): Promise<{ onboarding_url: string }> {
  return apiSend("POST", "/api/me/payouts/connect", { country });
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
