import type { Metadata } from "next";
import HomeClient from "./HomeClient";
import JsonLd from "./components/JsonLd";
import { itemListJsonLd } from "../lib/seo";
import { serverApiBase, type EventDto } from "../lib/api";

const API = serverApiBase();
const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

export const metadata: Metadata = {
  title: { absolute: "Underground — афіша подій і безкоштовні квитки" },
  description: "Концерти, вечірки, лекції й зустрічі поруч. Бронюй місця безкоштовно — квиток з QR одразу в телефоні.",
  alternates: { canonical: SITE },
  openGraph: {
    title: "Underground — афіша подій і безкоштовні квитки",
    description: "Бронюй місця на події поруч безкоштовно.",
    url: SITE,
    siteName: "Underground",
    type: "website",
  },
};

/** Published events, SSR-cached, for the home JSON-LD (crawlable structured data). */
async function fetchEvents(): Promise<EventDto[]> {
  try {
    const res = await fetch(`${API}/api/events`, {
      next: { revalidate: 300 },
      // Bounded: `next build` prerenders this page, and a slow API must not hang the image build.
      signal: AbortSignal.timeout(10_000),
    });
    if (!res.ok) return [];
    return (await res.json()) as EventDto[];
  } catch {
    return [];
  }
}

export default async function HomePage() {
  const events = await fetchEvents();
  return (
    <>
      {events.length > 0 && <JsonLd data={itemListJsonLd(events.slice(0, 30))} />}
      <HomeClient />
    </>
  );
}
