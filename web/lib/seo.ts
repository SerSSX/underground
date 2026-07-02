import type { EventDto } from "./api";

const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

/** schema.org/Event JSON-LD for an event detail page → Google rich event results (TASK-202). */
export function eventJsonLd(event: EventDto): Record<string, unknown> {
  const url = `${SITE}/events/${event.id}`;
  const address = [event.venue_address, event.city].filter(Boolean).join(", ") || event.city || "Україна";
  const jsonLd: Record<string, unknown> = {
    "@context": "https://schema.org",
    "@type": "Event",
    name: event.title,
    url,
    eventStatus: "https://schema.org/EventScheduled",
    eventAttendanceMode: "https://schema.org/OfflineEventAttendanceMode",
    location: { "@type": "Place", name: event.venue_address || event.city || "Underground", address },
    organizer: { "@type": "Organization", name: "Underground" },
    offers: {
      "@type": "Offer",
      price: event.price,
      priceCurrency: event.currency,
      availability: event.tickets_left > 0 ? "https://schema.org/InStock" : "https://schema.org/SoldOut",
      url,
    },
  };
  if (event.starts_at) jsonLd.startDate = event.starts_at;
  if (event.poster_url) jsonLd.image = [event.poster_url];
  if (event.description) jsonLd.description = event.description.slice(0, 300);
  return jsonLd;
}

/** schema.org/ItemList JSON-LD for a category landing page. */
export function itemListJsonLd(events: EventDto[]): Record<string, unknown> {
  return {
    "@context": "https://schema.org",
    "@type": "ItemList",
    itemListElement: events.map((e, i) => ({
      "@type": "ListItem",
      position: i + 1,
      url: `${SITE}/events/${e.id}`,
      name: e.title,
    })),
  };
}
