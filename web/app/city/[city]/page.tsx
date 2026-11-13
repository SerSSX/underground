import type { Metadata } from "next";
import Listing from "../../components/Listing";
import JsonLd from "../../components/JsonLd";
import { itemListJsonLd } from "../../../lib/seo";
import { serverApiBase, type EventDto } from "../../../lib/api";

const API = serverApiBase();
const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

/** Published events in a city (public endpoint, SSR-cached) for the geo landing page. */
async function fetchByCity(city: string): Promise<EventDto[]> {
  try {
    const res = await fetch(`${API}/api/events?city=${encodeURIComponent(city)}`, { next: { revalidate: 300 } });
    if (!res.ok) return [];
    return (await res.json()) as EventDto[];
  } catch {
    return [];
  }
}

export async function generateMetadata({ params }: { params: Promise<{ city: string }> }): Promise<Metadata> {
  const city = decodeURIComponent((await params).city);
  const title = `Події в місті ${city}`;
  const description = `Афіша подій у місті ${city}: концерти, вечірки, стендап та більше. Бронюй місця безкоштовно в Underground.`;
  const url = `${SITE}/city/${encodeURIComponent(city)}`;
  return {
    title,
    description,
    alternates: { canonical: url },
    openGraph: { title, description, url, siteName: "Underground", type: "website" },
  };
}

export default async function CityPage({ params }: { params: Promise<{ city: string }> }) {
  const city = decodeURIComponent((await params).city);
  const events = await fetchByCity(city);
  return (
    <>
      {events.length > 0 && <JsonLd data={itemListJsonLd(events)} />}
      <Listing city={city} events={events} />
    </>
  );
}
