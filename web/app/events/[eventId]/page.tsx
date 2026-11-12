import type { Metadata } from "next";
import EventDetailClient from "./EventDetailClient";
import JsonLd from "../../components/JsonLd";
import { eventJsonLd } from "../../../lib/seo";
import { serverApiBase, type EventDto } from "../../../lib/api";

const API = serverApiBase();
const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

/** Server-side event fetch for SSR + metadata (public endpoint, no auth). */
async function fetchEvent(id: string): Promise<EventDto | null> {
  try {
    const res = await fetch(`${API}/api/events/${id}`, { next: { revalidate: 60 } });
    if (!res.ok) return null;
    return (await res.json()) as EventDto;
  } catch {
    return null;
  }
}

/** Dynamic Open Graph / Twitter metadata so shared event links get rich previews. */
export async function generateMetadata({ params }: { params: Promise<{ eventId: string }> }): Promise<Metadata> {
  const { eventId } = await params;
  const event = await fetchEvent(eventId);
  if (!event) return { title: "Подія" };
  const title = event.title;
  const description = (event.description ?? [event.category, event.city].filter(Boolean).join(" · ")).slice(0, 180);
  const url = `${SITE}/events/${eventId}`;
  const images = event.poster_url ? [{ url: event.poster_url }] : [];
  return {
    title,
    description,
    alternates: { canonical: url },
    openGraph: { title, description, url, type: "website", siteName: "Underground", images },
    twitter: { card: "summary_large_image", title, description, images: images.map((i) => i.url) },
  };
}

export default async function EventPage({ params }: { params: Promise<{ eventId: string }> }) {
  const { eventId } = await params;
  const event = await fetchEvent(eventId);
  return (
    <>
      {event && <JsonLd data={eventJsonLd(event)} />}
      <EventDetailClient eventId={eventId} initialEvent={event} />
    </>
  );
}
