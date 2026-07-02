import type { MetadataRoute } from "next";
import { EVENT_CATEGORIES, serverApiBase } from "../lib/api";

const API = serverApiBase();
const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

export const revalidate = 3600;

/** sitemap.xml — the afisha + every published event, for discovery/SEO (TASK-017 M7). */
export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  let events: { id: string; starts_at: string | null; city: string | null }[] = [];
  try {
    const res = await fetch(`${API}/api/events`, { next: { revalidate: 3600 } });
    if (res.ok) events = (await res.json()) as typeof events;
  } catch {
    /* sitemap degrades to just the home page if the API is unreachable */
  }
  const eventUrls: MetadataRoute.Sitemap = events.map((e) => ({
    url: `${SITE}/events/${e.id}`,
    lastModified: e.starts_at ? new Date(e.starts_at) : undefined,
    changeFrequency: "daily",
    priority: 0.8,
  }));
  // Category landing pages (TASK-202) — SEO entry points for "<category> events".
  const categoryUrls: MetadataRoute.Sitemap = EVENT_CATEGORIES.map((c) => ({
    url: `${SITE}/c/${encodeURIComponent(c)}`,
    changeFrequency: "daily",
    priority: 0.6,
  }));
  // City landing pages (TASK-206) — geo SEO entry points, derived from cities that have events.
  const cities = [...new Set(events.map((e) => e.city).filter((c): c is string => !!c))];
  const cityUrls: MetadataRoute.Sitemap = cities.map((c) => ({
    url: `${SITE}/city/${encodeURIComponent(c)}`,
    changeFrequency: "daily",
    priority: 0.6,
  }));
  return [{ url: SITE, changeFrequency: "hourly", priority: 1 }, ...categoryUrls, ...cityUrls, ...eventUrls];
}
