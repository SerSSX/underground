"use client";

import Link from "next/link";
import EventCard from "./EventCard";
import type { EventDto } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";
import { Compass } from "./Icon";

/** The event grid used by the search-engine landing pages (/city/…, /c/…). */
export default function Listing({ title, subtitle, events }: { title: string; subtitle: string; events: EventDto[] }) {
  const { lang } = useLang();
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
