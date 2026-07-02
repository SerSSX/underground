"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import {
  getEvent,
  getEventReviews,
  postReview,
  reserveTicket,
  checkout,
  payTest,
  saveEvent,
  unsaveEvent,
  mySaves,
  eventSocial,
  similarEvents,
  createReport,
  getMe,
  logTelemetry,
  getToken,
  fmtPrice,
  isPast,
  mapsUrl,
  errorMessage,
  type EventDto,
  type EventReviews,
  type EventSocial,
  type UserDto,
} from "../../../lib/api";
import Link from "next/link";
import { useLang, tr, fmtDateL } from "../../../lib/i18n";
import EventCard from "../../components/EventCard";
import Modal from "../../components/Modal";

/** Whole years from a YYYY-MM-DD dob as of today. */
function ageFrom(dob: string): number {
  // Parse the YYYY-MM-DD as a *local* date — `new Date("2008-01-01")` is parsed as UTC
  // midnight, which in negative-offset timezones rolls back a day and can mis-compute age
  // right around a birthday. Appending a local time component forces local interpretation.
  const d = new Date(`${dob}T00:00:00`);
  const t = new Date();
  let age = t.getFullYear() - d.getFullYear();
  const m = t.getMonth() - d.getMonth();
  if (m < 0 || (m === 0 && t.getDate() < d.getDate())) age -= 1;
  return age;
}

export default function EventDetailClient({
  eventId,
  initialEvent,
}: {
  eventId: string;
  initialEvent: EventDto | null;
}) {
  const router = useRouter();
  const [event, setEvent] = useState<EventDto | null>(initialEvent);
  const [reviews, setReviews] = useState<EventReviews | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [rating, setRating] = useState(0);
  const [reviewText, setReviewText] = useState("");
  const [saved, setSaved] = useState(false);
  const [social, setSocial] = useState<EventSocial | null>(null);
  const [similar, setSimilar] = useState<EventDto[]>([]);
  const [me, setMe] = useState<UserDto | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [reportTarget, setReportTarget] = useState<{ type: "event" | "review"; id: string } | null>(null);
  const [reportReason, setReportReason] = useState("");
  const [notice, setNotice] = useState<string | null>(null);
  const { lang } = useLang();

  function load() {
    getEvent(eventId).then(setEvent).catch((e) => setError(errorMessage(e)));
    getEventReviews(eventId).then(setReviews).catch(() => {});
    eventSocial(eventId).then(setSocial).catch(() => {});
    similarEvents(eventId).then(setSimilar).catch(() => {});
    if (getToken()) {
      mySaves().then((l) => setSaved(l.some((e) => e.id === eventId))).catch(() => {});
      getMe().then(setMe).catch(() => {});
    }
  }
  useEffect(load, [eventId]);

  function toggleSave() {
    if (!getToken()) {
      router.push(`/login?next=/events/${eventId}`);
      return;
    }
    const now = !saved;
    setSaved(now);
    (now ? saveEvent(eventId) : unsaveEvent(eventId)).catch(() => setSaved(!now));
  }

  if (error && !event) return <main className="container narrow"><div className="error">{error}</div></main>;
  if (!event) return <main className="container narrow"><p className="muted">{tr(lang, "Завантаження…", "Loading…")}</p></main>;

  const free = event.cost_type.toLowerCase() === "free";
  const past = isPast(event.starts_at);
  const maps = mapsUrl(event.venue_address, event.city);
  const ageMin = event.age_rating ? parseInt(event.age_rating, 10) : 0;
  // Pre-check age only when we know the user (authed): block if dob missing or too young.
  const ageBlocked = ageMin > 0 && getToken() != null && !(me?.dob ? ageFrom(me.dob) >= ageMin : false);

  // Free → reserve immediately; paid → show an order/escrow summary first.
  async function startBuy() {
    if (!getToken()) {
      router.push(`/login?next=/events/${eventId}`);
      return;
    }
    setError(null);
    if (free) {
      setBusy(true);
      try {
        const t = await reserveTicket(eventId);
        router.push(`/me/tickets?new=${t.id}`);
      } catch (e) {
        setError(errorMessage(e));
      } finally {
        setBusy(false);
      }
    } else {
      setConfirming(true);
    }
  }

  async function confirmBuy() {
    setBusy(true);
    setError(null);
    try {
      const c = await checkout(eventId);
      // Test mode: simulate payment immediately, then show the ticket.
      const t = await payTest(c.order_id);
      router.push(`/me/tickets?new=${t.id}`);
    } catch (e) {
      setError(errorMessage(e));
      setConfirming(false);
    } finally {
      setBusy(false);
    }
  }

  async function sendReview() {
    setBusy(true);
    setError(null);
    try {
      await postReview(eventId, rating, reviewText);
      setRating(0);
      setReviewText("");
      load();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  function report(targetType: "event" | "review", targetId: string) {
    if (!getToken()) {
      router.push(`/login?next=/events/${eventId}`);
      return;
    }
    setReportReason("");
    setReportTarget({ type: targetType, id: targetId });
  }

  async function submitReport() {
    if (!reportTarget || !reportReason.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await createReport(reportTarget.type, reportTarget.id, reportReason.trim());
      setReportTarget(null);
      setNotice(tr(lang, "Дякуємо, скаргу надіслано.", "Thanks, your report was sent."));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function share() {
    const url = typeof window !== "undefined" ? window.location.href : "";
    const data = { title: event?.title, text: `${event?.title} — Underground`, url };
    if (navigator.share) {
      await navigator.share(data).catch(() => {});
    } else {
      await navigator.clipboard.writeText(url).catch(() => {});
      alert(tr(lang, "Посилання скопійовано", "Link copied"));
    }
    logTelemetry("share", { event_id: eventId });
  }

  function addToCalendar() {
    if (!event) return;
    const dt = (iso: string | null) => (iso ? `${iso.slice(0, 19).replace(/[-:]/g, "")}Z` : "");
    const ics = [
      "BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Underground//UA", "BEGIN:VEVENT",
      `UID:${event.id}@underground`,
      `SUMMARY:${event.title}`,
      event.starts_at ? `DTSTART:${dt(event.starts_at)}` : "",
      `LOCATION:${[event.venue_address, event.city].filter(Boolean).join(", ")}`,
      event.description ? `DESCRIPTION:${event.description.replace(/\n/g, " ")}` : "",
      "END:VEVENT", "END:VCALENDAR",
    ].filter(Boolean).join("\r\n");
    const a = document.createElement("a");
    a.href = `data:text/calendar;charset=utf-8,${encodeURIComponent(ics)}`;
    a.download = "event.ics";
    a.click();
  }

  return (
    <main className="container narrow detail">
      <div
        className="hero"
        style={{
          backgroundImage: event.poster_url
            ? `url(${event.poster_url})`
            : "linear-gradient(160deg, #2a2a33, #0b0b0d)",
        }}
      />
      <span className="pill">{event.category}</span>
      {event.age_rating && <span className="pill" style={{ marginLeft: 6 }}>{event.age_rating}+</span>}
      <h1 style={{ marginTop: 10 }}>{event.title}</h1>
      <div className="muted">{[event.city, fmtDateL(event.starts_at, lang)].filter(Boolean).join(" · ")}</div>

      {maps && (
        <div className="section">
          📍 {event.venue_address ?? event.city}{" "}
          <a className="link" href={maps} target="_blank" rel="noreferrer">{tr(lang, "Відкрити в Google Maps", "Open in Google Maps")}</a>
        </div>
      )}

      {event.description && <p className="section">{event.description}</p>}

      {social && (social.interested_count > 0 || social.going_count > 0) && (
        <div className="section">
          <span style={{ color: "var(--accent)", fontWeight: 600 }}>
            🔥 {tr(lang, `${social.interested_count} цікавляться · ${social.going_count} піде`, `${social.interested_count} interested · ${social.going_count} going`)}
          </span>
          {[...social.friends_going, ...social.friends_interested].length > 0 && (
            <div className="muted" style={{ fontSize: 13, marginTop: 2 }}>
              {tr(lang, "Серед них друзі:", "Friends among them:")}{" "}
              {Array.from(new Set([...social.friends_going, ...social.friends_interested].map((u) => u.name)))
                .slice(0, 4)
                .join(", ")}
            </div>
          )}
        </div>
      )}

      {event.gallery_urls.length > 0 && (
        <div className="section gallery">
          {event.gallery_urls.map((u) => <img key={u} src={u} alt={tr(lang, "фото події", "event photo")} />)}
        </div>
      )}
      {event.video_url && (
        <div className="section">
          ▶ <a className="link" href={event.video_url} target="_blank" rel="noreferrer">{tr(lang, "Дивитися відео", "Watch video")}</a>
        </div>
      )}
      {event.links.length > 0 && (
        <div className="section row gap" style={{ flexWrap: "wrap" }}>
          {event.links.map((l) => (
            <a key={l} className="pill" href={l} target="_blank" rel="noreferrer">{l.replace(/^https?:\/\//, "").split("/")[0]}</a>
          ))}
        </div>
      )}

      <div className="section row spread">
        <span className="price" style={{ fontSize: 24 }}>
          {free ? tr(lang, "Безкоштовно", "Free") : fmtPrice(event.price, event.currency)}
        </span>
        <span className="muted">{tr(lang, `Залишилось ${event.tickets_left}`, `${event.tickets_left} left`)}</span>
      </div>
      {!free && (
        <div className="pill ok" style={{ display: "block", fontSize: 12, marginBottom: 8 }}>
          {tr(lang, "🛡 Захищена покупка · кошти в escrow до події, повернення поки не завершилась", "🛡 Protected purchase · funds held in escrow until the event, refundable while uncleared")}
        </div>
      )}
      {ageMin > 0 && (
        <div className="muted" style={{ fontSize: 13 }}>
          {tr(lang, `Подія ${ageMin}+. Вік перевіряється за датою народження у профілі.`, `${ageMin}+ event. Age is verified by the date of birth in your profile.`)}
          {ageBlocked && <> <Link className="link" href="/account">{tr(lang, "Додати дату народження", "Add your date of birth")}</Link></>}
        </div>
      )}
      {error && <div className="error">{error}</div>}
      {notice && <div className="pill ok" style={{ display: "block", marginBottom: 8 }}>{notice}</div>}
      {confirming ? (
        <div className="section" style={{ border: "1px solid var(--border)", borderRadius: 12, padding: 12 }}>
          <div className="row spread"><span>{tr(lang, "До сплати", "Total")}</span><strong>{fmtPrice(event.price, event.currency)}</strong></div>
          <div className="pill ok" style={{ display: "block", marginTop: 6 }}>
            {tr(lang, "🛡 Гроші захищені: escrow тримає кошти до події", "🛡 Money protected: escrow holds funds until the event")}
          </div>
          <p className="muted" style={{ fontSize: 13, marginTop: 6 }}>
            {tr(lang, "Кошти тримаються в escrow і переходять організатору лише після події (≈5 днів). Повернення — поки кошти не очищені.", "Funds are held in escrow and released to the organizer only after the event (~5 days). Refunds available while funds are uncleared.")}{" "}
            <Link className="link" href="/refunds">{tr(lang, "Політика повернення", "Refund policy")}</Link>
          </p>
          <div className="row gap" style={{ marginTop: 8 }}>
            <button className="btn" onClick={confirmBuy} disabled={busy} style={{ flex: 1 }}>{busy ? "…" : tr(lang, "Підтвердити й сплатити", "Confirm & pay")}</button>
            <button className="btn ghost" onClick={() => setConfirming(false)} disabled={busy}>{tr(lang, "Скасувати", "Cancel")}</button>
          </div>
        </div>
      ) : (
        <div className="row gap" style={{ marginTop: 10 }}>
          <button className="btn" onClick={startBuy} disabled={busy || past || event.sales_closed || ageBlocked} style={{ flex: 1 }}>
            {past
              ? tr(lang, "Подія завершилась", "Event ended")
              : event.sales_closed
                ? tr(lang, "Продаж призупинено", "Sales paused")
                : ageBlocked
                  ? tr(lang, `Лише ${ageMin}+`, `${ageMin}+ only`)
                  : free
                    ? tr(lang, "Отримати квиток", "Get ticket")
                    : tr(lang, "Купити квиток", "Buy ticket")}
          </button>
          <button className="btn ghost" onClick={toggleSave} title={tr(lang, "Зберегти", "Save")}>
            {saved ? tr(lang, "🔖 Збережено", "🔖 Saved") : tr(lang, "📑 Зберегти", "📑 Save")}
          </button>
        </div>
      )}
      <div className="row gap" style={{ marginTop: 8 }}>
        <button className="btn ghost sm" onClick={share}>{tr(lang, "↗ Поділитися", "↗ Share")}</button>
        <button className="btn ghost sm" onClick={addToCalendar}>{tr(lang, "📅 В календар", "📅 Add to calendar")}</button>
        <button className="btn ghost sm" onClick={() => report("event", event.id)}>{tr(lang, "⚑ Поскаржитися", "⚑ Report")}</button>
      </div>

      <div className="section">
        <h2 style={{ fontSize: 18 }}>
          {tr(lang, "Відгуки", "Reviews")} {reviews?.avg_rating != null && <span className="muted">★ {reviews.avg_rating.toFixed(1)} · {reviews.count}</span>}
        </h2>
        {past && getToken() && (
          <div className="form" style={{ marginTop: 8 }}>
            <div className="stars" style={{ fontSize: 22, cursor: "pointer" }}>
              {[1, 2, 3, 4, 5].map((s) => (
                <span key={s} onClick={() => setRating(s)}>{s <= rating ? "★" : "☆"}</span>
              ))}
            </div>
            <textarea className="input" rows={2} placeholder={tr(lang, "Ваш відгук…", "Your review…")} value={reviewText} onChange={(e) => setReviewText(e.target.value)} />
            <button className="btn sm" onClick={sendReview} disabled={rating < 1 || busy}>{tr(lang, "Надіслати", "Submit")}</button>
          </div>
        )}
        {!past && <p className="muted">{tr(lang, "Відгуки відкриються після початку події.", "Reviews open after the event starts.")}</p>}
        {reviews?.reviews.map((r) => (
          <div key={r.id} className="review">
            <div className="row spread">
              <strong>{r.author_name}</strong>
              <span className="stars">{"★".repeat(r.rating)}</span>
            </div>
            {r.text && <div style={{ marginTop: 4 }}>{r.text}</div>}
            <button className="btn ghost sm" style={{ marginTop: 4 }} onClick={() => report("review", r.id)}>
              {tr(lang, "поскаржитися", "report")}
            </button>{" "}
          </div>
        ))}
      </div>

      {similar.length > 0 && (
        <div className="section">
          <h2 style={{ fontSize: 18 }}>{tr(lang, "Схожі події", "Similar events")}</h2>
          <div className="grid">
            {similar.map((e) => <EventCard key={e.id} event={e} />)}
          </div>
        </div>
      )}

      {reportTarget && (
        <Modal
          title={reportTarget.type === "review" ? tr(lang, "Поскаржитися на відгук", "Report review") : tr(lang, "Поскаржитися на подію", "Report event")}
          onClose={() => setReportTarget(null)}
        >
          <form className="form" onSubmit={(e) => { e.preventDefault(); submitReport(); }}>
            <textarea
              className="input"
              rows={3}
              placeholder={tr(lang, "Причина скарги…", "Reason for report…")}
              value={reportReason}
              onChange={(e) => setReportReason(e.target.value)}
              required
            />
            <button className="btn block" disabled={!reportReason.trim() || busy}>{tr(lang, "Надіслати", "Send")}</button>
          </form>
        </Modal>
      )}
    </main>
  );
}
