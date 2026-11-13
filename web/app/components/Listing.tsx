"use client";

import Link from "next/link";
import EventCard from "./EventCard";
import type { EventDto } from "../../lib/api";
import { useLang, tr, tf } from "../../lib/i18n";
import { categoryLabel } from "../../lib/ui";
import { Compass } from "./Icon";

/** The event grid used by the search-engine landing pages (/city/…, /c/…). The heading is worded here, in the
 *  reader's language (the server renders it in Ukrainian, then hydration switches it). */
export default function Listing({ category, city, events }: { category?: string; city?: string; events: EventDto[] }) {
  const { lang } = useLang();
  const label = category ? categoryLabel(category, lang) : "";
  const title = category ? label : tf(lang, "Події: {city}", "Events: {city}", { city });
  const subtitle = category
    ? tf(lang, "Події категорії «{category}» поруч. Бронюйте місця безкоштовно.", "{category} events near you. Book your places for free.", { category: label })
    : tf(lang, "Концерти, вечірки, лекції й зустрічі в місті {city}. Вхід вільний — бронюйте місця онлайн.", "Concerts, parties, talks and meetups in {city}. Free entry — book your places online.", { city });
  return (
    <main className="container">
      <div className="page-head">
        <div>
          <h1>{title}</h1>
          <p className="muted">{subtitle}</p>
        </div>
        <Link className="btn ghost" href="/"><Compass className="ico sm" />{tr(lang, "Уся афіша", "All events")}</Link>
      </div>
      {events.length === 0 ? (
        <div className="empty">
          <h3>{tr(lang, "Поки порожньо", "Nothing here yet")}</h3>
          <p>{tr(lang, "Подій тут ще немає — загляньте в загальну афішу.", "No events here yet — check the main listings.")}</p>
          <Link className="btn" href="/">{tr(lang, "До афіші", "Browse events")}</Link>
        </div>
      ) : (
        <div className="grid">{events.map((e, i) => <EventCard key={e.id} event={e} eager={i < 4} />)}</div>
      )}
    </main>
  );
}
