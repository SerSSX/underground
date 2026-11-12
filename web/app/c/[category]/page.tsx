import type { Metadata } from "next";
import Listing from "../../components/Listing";
import JsonLd from "../../components/JsonLd";
import { itemListJsonLd } from "../../../lib/seo";
import { serverApiBase, type EventDto } from "../../../lib/api";
import { categoryLabel } from "../../../lib/ui";

const API = serverApiBase();
const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

/** Published events in a category (public endpoint, SSR-cached) for the landing page. */
async function fetchByCategory(category: string): Promise<EventDto[]> {
  try {
    const res = await fetch(`${API}/api/events?category=${encodeURIComponent(category)}`, { next: { revalidate: 300 } });
    if (!res.ok) return [];
    return (await res.json()) as EventDto[];
  } catch {
    return [];
  }
}

export async function generateMetadata({ params }: { params: Promise<{ category: string }> }): Promise<Metadata> {
  const cat = decodeURIComponent((await params).category);
  const title = `${categoryLabel(cat, "uk")} — події поруч`;
  const description = `Афіша подій категорії «${cat}»: концерти, вечірки та більше. Бронюй місця безкоштовно в Underground.`;
  const url = `${SITE}/c/${encodeURIComponent(cat)}`;
  return {
    title,
    description,
    alternates: { canonical: url },
    openGraph: { title, description, url, siteName: "Underground", type: "website" },
  };
}

export default async function CategoryPage({ params }: { params: Promise<{ category: string }> }) {
  const cat = decodeURIComponent((await params).category);
  const events = await fetchByCategory(cat);
  return (
    <>
      {events.length > 0 && <JsonLd data={itemListJsonLd(events)} />}
      <Listing title={categoryLabel(cat, "uk")} subtitle={`Події категорії «${categoryLabel(cat, "uk")}» поруч. Бронюйте місця безкоштовно.`} events={events} />
    </>
  );
}
