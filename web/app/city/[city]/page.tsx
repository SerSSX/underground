import type { Metadata } from "next";
import Link from "next/link";
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
  const title = `Події в місті ${city} — Underground`;
  const description = `Афіша подій у місті ${city}: концерти, вечірки, стендап та більше. Купуй квитки онлайн в Underground.`;
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
    <main className="container narrow">
      {events.length > 0 && <JsonLd data={itemListJsonLd(events)} />}
      <h1 style={{ fontSize: 26, margin: "8px 0 6px" }}>Події в місті {city}</h1>
      <p className="muted" style={{ marginBottom: 14 }}>Афіша подій у місті {city} в Underground.</p>
      {events.length === 0 ? (
        <p className="muted">Поки немає подій у цьому місті. <Link href="/">Уся афіша →</Link></p>
      ) : (
        <ul style={{ listStyle: "none", padding: 0, display: "grid", gap: 10 }}>
          {events.map((e) => (
            <li key={e.id}>
              <Link href={`/events/${e.id}`} className="ticket" style={{ display: "block" }}>
                <strong>{e.title}</strong>
                <div className="muted" style={{ fontSize: 13 }}>
                  {[e.category, e.starts_at ? e.starts_at.slice(0, 10) : "TBA", `${e.price} ${e.currency}`]
                    .filter(Boolean)
                    .join(" · ")}
                </div>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}
