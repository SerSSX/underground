"use client";

import { useRouter } from "next/navigation";
import { fmtPrice, type EventDto } from "../../lib/api";
import { useLang, tr, fmtDateL } from "../../lib/i18n";

const COLORS: Record<string, string> = {
  Rock: "#c0392b", Metal: "#7f1d1d", Jazz: "#b8860b", Electronic: "#6d28d9", "Hip-Hop": "#9333ea",
  Poetry: "#0e7490", Yoga: "#15803d", "Board Games": "#0f766e", Party: "#be185d", Exhibition: "#b45309",
};
function color(cat: string): string {
  return COLORS[cat] ?? "#3a3a44";
}
export default function EventCard({
  event,
  saved = false,
  onToggleSave,
}: {
  event: EventDto;
  saved?: boolean;
  onToggleSave?: (id: string) => void;
}) {
  const router = useRouter();
  const { lang } = useLang();
  const free = event.cost_type.toLowerCase() === "free";
  const REASON: Record<string, string> = {
    followed: tr(lang, "★ ВИ ПІДПИСАНІ", "★ FOLLOWING"),
    interest: tr(lang, "ЗА ІНТЕРЕСОМ", "YOUR INTEREST"),
    city: tr(lang, "ПОРЯД З ВАМИ", "NEAR YOU"),
    trending: tr(lang, "🔥 ПОПУЛЯРНЕ", "🔥 TRENDING"),
  };
  const open = () => router.push(`/events/${event.id}`);
  return (
    <div
      className="ecard"
      role="link"
      tabIndex={0}
      aria-label={event.title}
      onClick={open}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") { e.preventDefault(); open(); }
      }}
    >
      <div
        className="poster"
        style={{
          backgroundImage: event.poster_url
            ? `url(${event.poster_url})`
            : `linear-gradient(160deg, ${color(event.category)}, #0b0b0d)`,
        }}
      >
        <span className="row" style={{ gap: 6 }}>
          <span className="tag">{event.category}</span>
          {event.age_rating && <span className="tag" style={{ background: "var(--danger, #e2553a)", fontWeight: 700 }}>{event.age_rating}+</span>}
        </span>
        {onToggleSave ? (
          <button
            className="savebtn"
            aria-label={saved ? tr(lang, "Збережено", "Saved") : tr(lang, "Зберегти", "Save")}
            aria-pressed={saved}
            title={saved ? tr(lang, "Збережено", "Saved") : tr(lang, "Зберегти", "Save")}
            onClick={(e) => {
              e.stopPropagation();
              onToggleSave(event.id);
            }}
          >
            {saved ? "🔖" : "📑"}
          </button>
        ) : (
          <span className="tag">{tr(lang, `лишилось ${event.tickets_left}`, `${event.tickets_left} left`)}</span>
        )}
      </div>
      <div className="body">
        {event.reason && <div className="reason">{REASON[event.reason] ?? ""}</div>}
        <h3>{event.title}</h3>
        <div className="muted" style={{ fontSize: 13, marginBottom: 6 }}>
          {[
            event.city,
            event.distance_km != null ? (event.distance_km < 1 ? tr(lang, "поряд", "nearby") : tr(lang, `~${Math.round(event.distance_km)} км`, `~${Math.round(event.distance_km)} km`)) : null,
            fmtDateL(event.starts_at, lang),
          ].filter(Boolean).join(" · ")}
        </div>
        <div className="row spread">
          <div className="price">
            {free ? tr(lang, "Безкоштовно", "Free") : fmtPrice(event.price, event.currency)}
            {!free && <span className="pill ok" style={{ fontSize: 11, marginLeft: 6 }}>🛡 escrow</span>}
          </div>
          {!!event.saves_count && event.saves_count > 0 && (
            <span className="muted" style={{ fontSize: 12 }}>{tr(lang, `${event.saves_count} цікавляться`, `${event.saves_count} interested`)}</span>
          )}
        </div>
      </div>
    </div>
  );
}
