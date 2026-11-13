"use client";

import Link from "next/link";
import { fmtPrice, type EventDto } from "../../lib/api";
import { FREE_ONLY } from "../../lib/flags";
import { useLang, tr, tf, tp } from "../../lib/i18n";
import { kmShown } from "../../lib/geo-core.mjs";
import { categoryLabel, dateBadge, timeOf } from "../../lib/ui";
import Poster from "./Poster";
import { Heart, MapPin } from "./Icon";

export default function EventCard({
  event,
  saved = false,
  onToggleSave,
  eager = false,
  distanceKm = null,
}: {
  event: EventDto;
  saved?: boolean;
  /** Absent → no heart (e.g. similar events). Signed-out callers pass a function that sends to login. */
  onToggleSave?: (id: string) => void;
  eager?: boolean;
  /** Distance from the visitor ("near me"), shown as "~3 km". */
  distanceKm?: number | null;
}) {
  const { lang } = useLang();
  const free = FREE_ONLY || event.cost_type.toLowerCase() === "free";
  const badge = dateBadge(event.starts_at, lang, event.timezone);
  const left = event.tickets_left;
  const away = distanceKm != null && Number.isFinite(distanceKm) ? tf(lang, "~{n} км", "~{n} km", { n: kmShown(distanceKm) }) : null;
  const place = [timeOf(event.starts_at, event.timezone), event.venue_address ? event.venue_address.split(",")[0] : null, event.city, away]
    .filter(Boolean)
    .join(" · ");

  return (
    <article className="ecard">
      <Link href={`/events/${event.id}`} className="poster" aria-label={event.title}>
        <Poster src={event.poster_url} title={event.title} category={event.category} eager={eager} variant="card" />
        {badge && (
          <span className="date-badge" aria-hidden>
            <div className="d">{badge.day}</div>
            <div className="m">{badge.mon}</div>
          </span>
        )}
        <span className="poster-tags">
          <span className="tag">{categoryLabel(event.category, lang)}</span>
          {event.age_rating && <span className="tag age">{event.age_rating}+</span>}
        </span>
      </Link>
      {onToggleSave && (
        <button
          className={`round-btn save ${saved ? "on" : ""}`}
          aria-pressed={saved}
          aria-label={saved ? tr(lang, "Прибрати зі збережених", "Remove from saved") : tr(lang, "Зберегти", "Save")}
          onClick={() => onToggleSave(event.id)}
        >
          <Heart className="ico sm" />
        </button>
      )}
      <Link href={`/events/${event.id}`} className="body">
        <h3>{event.title}</h3>
        {place && <div className="meta"><MapPin className="ico sm" aria-hidden />{place}</div>}
        <div className="foot">
          {left <= 0 ? (
            <span className="badge danger">{tr(lang, "Місць немає", "Full")}</span>
          ) : free ? (
            <span className="badge ok">{tr(lang, "Безкоштовно", "Free")}</span>
          ) : (
            <span className="badge accent">{fmtPrice(event.price, event.currency)}</span>
          )}
          {left > 0 && left <= 10 ? (
            <span className="xs" style={{ color: "var(--warn-fg)" }}>
              {tp(lang, left, { one: "лишилось {n} місце", few: "лишилось {n} місця", many: "лишилось {n} місць" }, { one: "{n} left", other: "{n} left" })}
            </span>
          ) : !!event.saves_count && event.saves_count > 0 ? (
            <span className="xs subtle">{tp(lang, event.saves_count, { one: "{n} цікавиться", few: "{n} цікавляться", many: "{n} цікавляться" }, { one: "{n} interested", other: "{n} interested" })}</span>
          ) : null}
        </div>
      </Link>
    </article>
  );
}

export function EventCardSkeleton() {
  return (
    <div className="ecard" aria-hidden>
      <div className="skeleton" style={{ aspectRatio: "4 / 3" }} />
      <div className="skeleton" style={{ height: 18, width: "80%", borderRadius: 6 }} />
      <div className="skeleton" style={{ height: 14, width: "55%", borderRadius: 6 }} />
    </div>
  );
}
