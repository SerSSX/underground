import type { MetadataRoute } from "next";

const API = process.env.NEXT_PUBLIC_API_BASE_URL || "http://localhost:8080";
const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

export const revalidate = 3600;

/** sitemap.xml — the afisha + every published event, for discovery/SEO (TASK-017 M7). */
export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  let events: { id: string; starts_at: string | null }[] = [];
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
  return [{ url: SITE, changeFrequency: "hourly", priority: 1 }, ...eventUrls];
}
