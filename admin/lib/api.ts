const BASE = process.env.NEXT_PUBLIC_API_BASE_URL || "http://localhost:8080";

const TOKEN_KEY = "admin_token";

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

export async function login(email: string, password: string): Promise<string> {
  const res = await fetch(`${BASE}/api/auth/login`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ email, password }),
  });
  if (!res.ok) {
    throw new Error(res.status === 401 ? "Невірний email або пароль" : `Помилка ${res.status}`);
  }
  const data = (await res.json()) as { token: string };
  return data.token;
}

export interface PaymentSettings {
  mode: string;
  provider: string;
  ls_store_id: string | null;
  ls_variant_id: string | null;
  ls_api_key_set: boolean;
  // Publishable keys are safe to show; secrets come back only as a boolean, never a value.
  stripe_publishable_key: string | null;
  stripe_secret_key_set: boolean;
  stripe_webhook_secret_set: boolean;
  stripe_key_kind: string | null;
  platform_fee_bps: number;
  updated_at: string;
}

export interface PaymentReadiness {
  mode: string;
  provider: string;
  stripe_secret_key_set: boolean;
  stripe_webhook_secret_set: boolean;
  stripe_key_kind: string | null;
  provider_recognised: boolean;
  ready_for_live: boolean;
}

export interface ConnectionCheck {
  ok: boolean;
  account_id: string | null;
  message: string;
}

export async function getReadiness(): Promise<PaymentReadiness> {
  return apiGet<PaymentReadiness>("/api/admin/payment-readiness");
}

/// Proves the stored key actually opens a Stripe account, before a buyer finds out it doesn't.
export async function testStripe(): Promise<ConnectionCheck> {
  return apiSend<ConnectionCheck>("POST", "/api/admin/stripe/test");
}

export async function getSettings(): Promise<PaymentSettings> {
  const res = await fetch(`${BASE}/api/admin/payment-settings`, { headers: authHeaders() });
  if (res.status === 401) throw new Error("Сесія недійсна — увійдіть знову");
  if (res.status === 403) throw new Error("Потрібні права адміністратора");
  if (!res.ok) throw new Error(`Помилка ${res.status}`);
  return (await res.json()) as PaymentSettings;
}

export interface UpdateSettings {
  mode?: string;
  provider?: string;
  platform_fee_bps?: number;
  ls_store_id?: string;
  ls_variant_id?: string;
  ls_api_key?: string;
  stripe_secret_key?: string;
  stripe_publishable_key?: string;
  stripe_webhook_secret?: string;
}

export async function updateSettings(body: UpdateSettings): Promise<PaymentSettings> {
  const res = await fetch(`${BASE}/api/admin/payment-settings`, {
    method: "PUT",
    headers: { ...authHeaders(), "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`Помилка ${res.status}`);
  return (await res.json()) as PaymentSettings;
}

export function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : "Невідома помилка";
}

async function check(res: Response): Promise<Response> {
  if (res.status === 401) throw new Error("Сесія недійсна — увійдіть знову");
  if (res.status === 403) throw new Error("Недостатньо прав");
  if (!res.ok) {
    const body = (await res.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error || `Помилка ${res.status}`);
  }
  return res;
}

export async function apiGet<T>(path: string): Promise<T> {
  const res = await check(await fetch(`${BASE}${path}`, { headers: authHeaders() }));
  return (await res.json()) as T;
}

export async function apiSend<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await check(
    await fetch(`${BASE}${path}`, {
      method,
      headers: { ...authHeaders(), "content-type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    }),
  );
  // Tolerate empty (204) bodies — some admin endpoints return No Content.
  const text = await res.text();
  return (text ? JSON.parse(text) : null) as T;
}

export interface AdminStats {
  users_count: number;
  events_count: number;
  tickets_count: number;
  gmv: string;
  platform_fee: string;
}

export interface AdminUser {
  id: string;
  email: string;
  name: string;
  role: string;
  blocked: boolean;
  verified: boolean;
  kyc_status: string;
  created_at: string;
}

/** Admin sets an organizer's KYC decision (P1 #78). */
export async function setKyc(id: string, status: string): Promise<void> {
  await apiSend("POST", `/api/admin/users/${id}/kyc`, { status });
}

/** Abuse report in the moderation queue (P1 #76). */
export interface Report {
  id: string;
  reporter_id: string;
  reporter_email: string | null;
  target_type: string;
  target_id: string;
  reason: string;
  details: string | null;
  status: string;
  created_at: string;
}
export async function listReports(status?: string): Promise<Report[]> {
  return apiGet<Report[]>(`/api/admin/reports${status ? `?status=${status}` : ""}`);
}
export async function resolveReport(id: string, status: string, note?: string): Promise<void> {
  await apiSend("PATCH", `/api/admin/reports/${id}`, { status, note: note || null });
}

// ---- moderation audit-trail ----
export interface AdminActionLog {
  id: string;
  admin_name: string | null;
  admin_email: string | null;
  action: string;
  target_type: string;
  target_id: string | null;
  detail: Record<string, unknown>;
  created_at: string;
}
export async function listAdminActions(): Promise<AdminActionLog[]> {
  return apiGet<AdminActionLog[]>("/api/admin/actions");
}

// Moderation enforcement actions taken from the reports queue.
export async function blockUser(id: string): Promise<void> {
  await apiSend("PATCH", `/api/admin/users/${id}`, { blocked: true });
}
export async function moderateEvent(id: string, status: string): Promise<void> {
  await apiSend("PATCH", `/api/admin/events/${id}`, { status });
}
export async function adminDeletePost(id: string): Promise<void> {
  await apiSend("DELETE", `/api/admin/posts/${id}`);
}
export async function adminDeleteReview(id: string): Promise<void> {
  await apiSend("DELETE", `/api/admin/reviews/${id}`);
}

export interface Payout {
  id: string;
  organizer_id: string;
  organizer_email: string | null;
  amount: string;
  currency: string;
  order_count: number;
  status: string;
  failure_reason: string | null;
  created_at: string;
}
export async function listPayouts(status?: string): Promise<Payout[]> {
  return apiGet<Payout[]>(`/api/admin/payouts${status ? `?status=${status}` : ""}`);
}
export async function updatePayout(id: string, status: string, failureReason?: string): Promise<void> {
  await apiSend("PATCH", `/api/admin/payouts/${id}`, { status, failure_reason: failureReason || null });
}

export interface AdminEvent {
  id: string;
  title: string;
  status: string;
  category: string;
  city: string | null;
  organizer_email: string | null;
  starts_at: string | null;
  tickets_left: number;
  sales_closed: boolean;
}

export interface AdminOrder {
  id: string;
  event_title: string | null;
  buyer_email: string | null;
  amount: string;
  currency: string;
  status: string;
  escrow_status: string | null;
  created_at: string;
}

export interface OrganizerStats {
  events_count: number;
  tickets_sold: number;
  tickets_validated: number;
  revenue: string;
  escrow_held: string;
  escrow_cleared: string;
}

export interface OrganizerEvent {
  id: string;
  title: string;
  status: string;
  starts_at: string | null;
  price: string;
  currency: string;
  tickets_left: number;
  sales_closed: boolean;
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

export interface Me {
  id: string;
  email: string;
  name: string;
  role: string;
  is_organizer: boolean;
}

export interface FullEvent {
  id: string;
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
}

export async function getMe(): Promise<Me> {
  return apiGet<Me>("/api/me");
}

export async function myEvents(): Promise<FullEvent[]> {
  return apiGet<FullEvent[]>("/api/me/events");
}

export async function createEvent(body: EventInput): Promise<FullEvent> {
  return apiSend<FullEvent>("POST", "/api/events", body);
}

export async function updateEvent(id: string, body: Partial<EventInput> & { status?: string; sales_closed?: boolean }): Promise<FullEvent> {
  return apiSend<FullEvent>("PUT", `/api/events/${id}`, body);
}

export async function cancelEvent(id: string): Promise<void> {
  await check(await fetch(`${BASE}/api/events/${id}`, { method: "DELETE", headers: authHeaders() }));
}

export async function uploadImage(file: File): Promise<string> {
  const form = new FormData();
  form.append("file", file);
  const res = await check(await fetch(`${BASE}/api/uploads`, { method: "POST", headers: authHeaders(), body: form }));
  const data = (await res.json()) as { url: string };
  return data.url;
}

/** datetime-local value ("2026-06-20T19:00") → RFC3339 UTC. Empty → null. */
export function toIso(local: string): string | null {
  if (!local) return null;
  return local.length === 16 ? `${local}:00Z` : `${local}Z`;
}

export const EVENT_CATEGORIES = [
  "Concert", "Rock", "Metal", "Electronic", "Hip-Hop", "Jazz", "Pop", "Folk", "Classical",
  "DJ Set", "Festival", "Karaoke", "Stand-Up", "Theatre", "Improv", "Dance", "Poetry", "Cinema",
  "Exhibition", "Photography", "Fashion", "Literature", "Craft", "Lecture", "Conference", "Workshop",
  "Course", "Networking", "Startup", "Hackathon", "Sport", "Yoga", "Hike", "Run", "Cycling", "Extreme",
  "Party", "Meetup", "Charity", "Family", "Food & Drink", "Tasting", "Board Games", "Quiz", "Market",
  "Gaming", "Spiritual", "Other",
];
