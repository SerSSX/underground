"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import {
  getEvent, getEventReviews, postReview, reserveTickets, saveEvent, unsaveEvent, mySaves, eventSocial,
  similarEvents, createReport, getProfile, myTickets, patchMe, logTelemetry, getToken, isPast, mapsUrl,
  errorMessage, fmtPrice,
  type EventDto, type EventReviews, type EventSocial, type PublicProfile,
} from "../../../lib/api";
import { FREE_ONLY, MAX_PLACES } from "../../../lib/flags";
import { useLang, tr } from "../../../lib/i18n";
import { useSession } from "../../../lib/session";
import { categoryLabel, longDate, timeOf, initials, placesLabel, plural } from "../../../lib/ui";
import EventCard from "../../components/EventCard";
import Modal from "../../components/Modal";
import Poster from "../../components/Poster";
import { Toast, useToast } from "../../components/Toast";
import {
  Calendar, MapPin, Heart, Share2, CalendarPlus, Flag, MessageCircle, Minus, Plus, Ticket, Navigation,
  ExternalLink, CircleCheck, Users, ChevronLeft,
} from "../../components/Icon";

/** Whole years from a YYYY-MM-DD date of birth, in local time. */
function ageFrom(dob: string): number {
  const d = new Date(`${dob}T00:00:00`);
  const t = new Date();
  let age = t.getFullYear() - d.getFullYear();
  const m = t.getMonth() - d.getMonth();
  if (m < 0 || (m === 0 && t.getDate() < d.getDate())) age -= 1;
  return age;
}

export default function EventDetailClient({ eventId, initialEvent }: { eventId: string; initialEvent: EventDto | null }) {
  const router = useRouter();
  const { lang } = useLang();
  const { me, refresh } = useSession();
  const [event, setEvent] = useState<EventDto | null>(initialEvent);
  const [organizer, setOrganizer] = useState<PublicProfile | null>(null);
  const [reviews, setReviews] = useState<EventReviews | null>(null);
  const [social, setSocial] = useState<EventSocial | null>(null);
  const [similar, setSimilar] = useState<EventDto[]>([]);
  const [saved, setSaved] = useState(false);
  const [mine, setMine] = useState(0);
  const [qty, setQty] = useState(1);
  const [booking, setBooking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dob, setDob] = useState("");
  const [rating, setRating] = useState(0);
  const [reviewText, setReviewText] = useState("");
  const [reportOpen, setReportOpen] = useState<{ type: "event" | "review"; id: string } | null>(null);
  const [reportReason, setReportReason] = useState("");
  const [toast, showToast] = useToast();

  function load() {
    getEvent(eventId).then(setEvent).catch((e) => setError(errorMessage(e)));
    getEventReviews(eventId).then(setReviews).catch(() => {});
    eventSocial(eventId).then(setSocial).catch(() => {});
    similarEvents(eventId).then(setSimilar).catch(() => {});
    if (getToken()) {
      mySaves().then((l) => setSaved(l.some((e) => e.id === eventId))).catch(() => {});
      myTickets().then((l) => setMine(l.filter((t) => t.event_id === eventId && t.status === "active").length)).catch(() => {});
    }
  }
  useEffect(load, [eventId]);
  useEffect(() => {
    if (event?.organizer_id) getProfile(event.organizer_id).then(setOrganizer).catch(() => {});
  }, [event?.organizer_id]);

  if (!event) {
    return (
      <main className="container">
        {error ? <div className="alert error">{error}</div> : <div className="skeleton" style={{ aspectRatio: "21 / 9" }} />}
      </main>
    );
  }

  const free = FREE_ONLY || event.cost_type.toLowerCase() === "free";
  const past = isPast(event.starts_at);
  const maps = mapsUrl(event.venue_address, event.city);
  const ageMin = event.age_rating ? parseInt(event.age_rating, 10) : 0;
  const left = event.tickets_left;
  const isOwner = !!me && me.id === event.organizer_id;
  const canMore = Math.max(0, Math.min(MAX_PLACES - mine, left));
  const needDob = ageMin > 0 && !!me && !me.dob;
  const tooYoung = ageMin > 0 && !!me?.dob && ageFrom(me.dob) < ageMin;
  const login = () => router.push(`/login?next=${encodeURIComponent(`/events/${eventId}`)}`);

  let blocked: string | null = null;
  if (past) blocked = tr(lang, "Подія вже минула", "This event has ended");
  else if (event.sales_closed) blocked = tr(lang, "Бронювання закрите", "Booking is closed");
  else if (left <= 0) blocked = tr(lang, "Усі місця заброньовані", "All places are booked");
  else if (tooYoung) blocked = tr(lang, `Подія лише для ${ageMin}+`, `${ageMin}+ only`);
  else if (me && canMore === 0) blocked = tr(lang, `У вас ${placesLabel(mine, lang)} — це максимум`, `You have ${mine} — that's the maximum`);

  function openBooking() {
    if (!me) return login();
    setError(null);
    setQty(1);
    setBooking(true);
  }

  async function book() {
    setBusy(true);
    setError(null);
    try {
      if (needDob) {
        if (!dob) throw new Error(tr(lang, "Вкажіть дату народження", "Enter your date of birth"));
        await patchMe({ dob });
        refresh();
        if (ageFrom(dob) < ageMin) throw new Error(tr(lang, `Ця подія лише для ${ageMin}+`, `This event is ${ageMin}+ only`));
      }
      await reserveTickets(eventId, qty);
      logTelemetry("ticket_booked", { event_id: eventId, quantity: qty });
      router.push(`/me/tickets?booked=${eventId}&n=${qty}`);
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  }

  function toggleSave() {
    if (!me) return login();
    const now = !saved;
    setSaved(now);
    (now ? saveEvent(eventId) : unsaveEvent(eventId)).catch(() => setSaved(!now));
  }

  async function share() {
    const url = window.location.href;
    if (navigator.share) {
      await navigator.share({ title: event?.title, url }).catch(() => {});
    } else {
      await navigator.clipboard.writeText(url).catch(() => {});
      showToast(tr(lang, "Посилання скопійовано", "Link copied"));
    }
    logTelemetry("share", { event_id: eventId });
  }

  function addToCalendar() {
    if (!event) return;
    const dt = (iso: string | null) => (iso ? `${new Date(iso).toISOString().replace(/[-:]/g, "").slice(0, 15)}Z` : "");
    const esc = (s: string) => s.replace(/[\\;,]/g, (c) => `\\${c}`).replace(/\n/g, "\\n");
    const ics = [
      "BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Underground//UA", "BEGIN:VEVENT",
      `UID:${event.id}@underground`, `SUMMARY:${esc(event.title)}`,
      event.starts_at ? `DTSTART:${dt(event.starts_at)}` : "",
      `LOCATION:${esc([event.venue_address, event.city].filter(Boolean).join(", "))}`,
      `URL:${window.location.href}`,
      "END:VEVENT", "END:VCALENDAR",
    ].filter(Boolean).join("\r\n");
    const a = document.createElement("a");
    a.href = `data:text/calendar;charset=utf-8,${encodeURIComponent(ics)}`;
    a.download = "underground-event.ics";
    a.click();
  }

  async function sendReview() {
    setBusy(true);
    try {
      await postReview(eventId, rating, reviewText);
      setRating(0);
      setReviewText("");
      load();
      showToast(tr(lang, "Дякуємо за відгук", "Thanks for the review"));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function submitReport() {
    if (!reportOpen || !reportReason.trim()) return;
    setBusy(true);
    try {
      await createReport(reportOpen.type, reportOpen.id, reportReason.trim());
      setReportOpen(null);
      showToast(tr(lang, "Скаргу надіслано, дякуємо", "Report sent, thank you"));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const priceLabel = free ? tr(lang, "Безкоштовно", "Free") : fmtPrice(event.price, event.currency);
  const leftLabel = left <= 0
    ? tr(lang, "Місць немає", "Full")
    : tr(lang, `Вільно ${left} ${plural(left, ["місце", "місця", "місць"])}`, `${left} places left`);

  const bookButton = (block = true) =>
    isOwner ? (
      <Link href={`/organizer/${event.id}`} className={`btn ${block ? "block lg" : ""}`}><Users className="ico sm" />{tr(lang, "Керувати подією", "Manage event")}</Link>
    ) : (
      <button className={`btn ${block ? "block lg primary-glow" : ""}`} onClick={openBooking} disabled={!!blocked}>
        {!blocked && <Ticket className="ico sm" />}
        {blocked ?? (mine > 0 ? tr(lang, "Забронювати ще", "Book more") : tr(lang, "Забронювати", "Book a place"))}
      </button>
    );

  return (
    <main className="container has-cta">
      <button className="btn quiet sm" style={{ marginLeft: -12, marginBottom: 8 }} onClick={() => (window.history.length > 1 ? router.back() : router.push("/"))}>
        <ChevronLeft className="ico sm" />{tr(lang, "Назад", "Back")}
      </button>

      <div className="event-hero">
        <Poster src={event.poster_url} title={event.title} category={event.category} eager />
      </div>

      <div className="event-layout">
        <div>
          <div className="row gap wrap">
            <span className="badge accent">{categoryLabel(event.category, lang)}</span>
            {event.age_rating && <span className="badge danger">{event.age_rating}+</span>}
            {mine > 0 && <span className="badge ok"><CircleCheck className="ico" />{tr(lang, `У вас ${placesLabel(mine, lang)}`, `You have ${mine}`)}</span>}
          </div>
          <h1 className="event-title">{event.title}</h1>

          <div className="info-list">
            {event.starts_at && (
              <div className="info-item">
                <span className="ib"><Calendar className="ico" /></span>
                <div><div className="t">{longDate(event.starts_at, lang, event.timezone)}</div><div className="s">{tr(lang, "Початок о", "Starts at")} {timeOf(event.starts_at, event.timezone)}</div></div>
              </div>
            )}
            {(event.venue_address || event.city) && (
              <div className="info-item">
                <span className="ib"><MapPin className="ico" /></span>
                <div>
                  <div className="t">{event.venue_address ?? event.city}</div>
                  <div className="s">
                    {event.city}
                    {maps && <> · <a className="link" href={maps} target="_blank" rel="noreferrer">{tr(lang, "Маршрут", "Directions")} <Navigation className="ico sm" style={{ verticalAlign: -3 }} /></a></>}
                  </div>
                </div>
              </div>
            )}
            {social && (social.interested_count > 0 || social.going_count > 0) && (
              <div className="info-item">
                <span className="ib"><Users className="ico" /></span>
                <div>
                  <div className="t">{tr(lang, `${social.going_count} підуть · ${social.interested_count} цікавляться`, `${social.going_count} going · ${social.interested_count} interested`)}</div>
                  {[...social.friends_going, ...social.friends_interested].length > 0 && (
                    <div className="s">{tr(lang, "Серед них: ", "Including: ")}{Array.from(new Set([...social.friends_going, ...social.friends_interested].map((u) => u.name))).slice(0, 3).join(", ")}</div>
                  )}
                </div>
              </div>
            )}
          </div>

          {event.description && (
            <section className="section">
              <h2 style={{ marginBottom: 10 }}>{tr(lang, "Про подію", "About")}</h2>
              <p className="prose">{event.description}</p>
            </section>
          )}

          {event.gallery_urls.length > 0 && (
            <div className="section gallery">
              {event.gallery_urls.map((u) => <img key={u} src={u} alt={tr(lang, "фото події", "event photo")} loading="lazy" />)}
            </div>
          )}
          {(event.video_url || event.links.length > 0) && (
            <div className="section row gap wrap">
              {event.video_url && <a className="btn secondary sm" href={event.video_url} target="_blank" rel="noreferrer">{tr(lang, "Відео", "Video")}<ExternalLink className="ico sm" /></a>}
              {event.links.map((l) => (
                <a key={l} className="btn secondary sm" href={l} target="_blank" rel="noreferrer">{l.replace(/^https?:\/\//, "").split("/")[0]}<ExternalLink className="ico sm" /></a>
              ))}
            </div>
          )}

          {organizer && (
            <section className="section card">
              <div className="muted xs strong" style={{ textTransform: "uppercase", letterSpacing: ".06em", marginBottom: 12 }}>{tr(lang, "Організатор", "Organiser")}</div>
              <div className="organizer-card">
                <span className="avatar lg">{organizer.avatar_url ? <img src={organizer.avatar_url} alt="" /> : initials(organizer.name)}</span>
                <div className="grow">
                  <div className="strong">{organizer.name}{organizer.verified && <CircleCheck className="ico sm" style={{ color: "var(--accent-fg)", marginLeft: 6, verticalAlign: -3 }} aria-label={tr(lang, "перевірений", "verified")} />}</div>
                  <div className="muted small">
                    {tr(lang, `${organizer.events_count} ${plural(organizer.events_count, ["подія", "події", "подій"])}`, `${organizer.events_count} events`)}
                    {organizer.rating_avg != null && <> · ★ {organizer.rating_avg.toFixed(1)}</>}
                  </div>
                </div>
                {!isOwner && (
                  <Link className="btn secondary sm" href={me ? `/inbox?to=${organizer.id}` : `/login?next=${encodeURIComponent(`/inbox?to=${organizer.id}`)}`}>
                    <MessageCircle className="ico sm" />{tr(lang, "Написати", "Message")}
                  </Link>
                )}
              </div>
            </section>
          )}

          <div className="section row gap wrap">
            <button className="btn ghost sm" onClick={share}><Share2 className="ico sm" />{tr(lang, "Поділитися", "Share")}</button>
            <button className="btn ghost sm" onClick={addToCalendar}><CalendarPlus className="ico sm" />{tr(lang, "У календар", "Add to calendar")}</button>
            <button className="btn quiet sm" onClick={() => (me ? setReportOpen({ type: "event", id: event.id }) : login())}><Flag className="ico sm" />{tr(lang, "Поскаржитися", "Report")}</button>
          </div>

          {(past || (reviews && reviews.count > 0)) && (
            <section className="section">
              <h2>{tr(lang, "Відгуки", "Reviews")} {reviews?.avg_rating != null && <span className="muted" style={{ fontSize: 15 }}>★ {reviews.avg_rating.toFixed(1)} · {reviews.count}</span>}</h2>
              {past && me && (
                <div className="card" style={{ marginTop: 12 }}>
                  <div className="form">
                    <div className="stars" style={{ fontSize: 26, cursor: "pointer" }} role="radiogroup" aria-label={tr(lang, "Оцінка", "Rating")}>
                      {[1, 2, 3, 4, 5].map((s) => (
                        <span key={s} role="radio" aria-checked={s === rating} tabIndex={0} onClick={() => setRating(s)} onKeyDown={(e) => e.key === "Enter" && setRating(s)}>{s <= rating ? "★" : "☆"}</span>
                      ))}
                    </div>
                    <textarea className="input" rows={2} placeholder={tr(lang, "Як усе пройшло?", "How was it?")} value={reviewText} onChange={(e) => setReviewText(e.target.value)} />
                    <button className="btn sm" style={{ alignSelf: "flex-start" }} onClick={sendReview} disabled={rating < 1 || busy}>{tr(lang, "Надіслати відгук", "Post review")}</button>
                  </div>
                </div>
              )}
              {reviews?.reviews.map((r) => (
                <div key={r.id} className="review">
                  <div className="row spread"><strong>{r.author_name}</strong><span className="stars">{"★".repeat(r.rating)}</span></div>
                  {r.text && <p style={{ marginTop: 6 }}>{r.text}</p>}
                  <button className="btn quiet sm" style={{ marginTop: 4, marginLeft: -12 }} onClick={() => (me ? setReportOpen({ type: "review", id: r.id }) : login())}>{tr(lang, "Поскаржитися", "Report")}</button>
                </div>
              ))}
            </section>
          )}
        </div>

        <aside className="booking desktop-only" aria-label={tr(lang, "Бронювання", "Booking")}>
          <div className="row spread">
            <span className="free">{priceLabel}</span>
            <button className={`round-btn ${saved ? "on" : ""}`} style={{ background: "var(--surface-2)" }} aria-pressed={saved} aria-label={tr(lang, "Зберегти", "Save")} onClick={toggleSave}><Heart className="ico sm" /></button>
          </div>
          {event.starts_at && <div className="when" style={{ marginTop: 10 }}>{longDate(event.starts_at, lang, event.timezone)} · {timeOf(event.starts_at, event.timezone)}</div>}
          <div className="muted small" style={{ marginTop: 2 }}>{[event.venue_address, event.city].filter(Boolean).join(", ")}</div>
          <hr />
          <div className="row spread small" style={{ marginBottom: 14 }}>
            <span className="muted">{leftLabel}</span>
            {mine > 0 && <Link className="link" href="/me/tickets">{tr(lang, "Мої квитки", "My tickets")}</Link>}
          </div>
          {bookButton()}
          <p className="subtle xs" style={{ marginTop: 12, textAlign: "center" }}>
            {tr(lang, `До ${MAX_PLACES} місць на людину · QR-квиток одразу · скасувати можна до початку`, `Up to ${MAX_PLACES} places each · QR ticket at once · cancel any time before the start`)}
          </p>
        </aside>
      </div>

      {similar.length > 0 && (
        <section className="section">
          <div className="section-head"><h2>{tr(lang, "Схожі події", "Similar events")}</h2></div>
          <div className="hscroll">{similar.map((e) => <EventCard key={e.id} event={e} />)}</div>
        </section>
      )}

      <div className="mobile-cta">
        <div className="grow">
          <div className="strong" style={{ color: free ? "var(--ok-fg)" : undefined }}>{priceLabel}</div>
          <div className="muted xs">{leftLabel}</div>
        </div>
        <button className={`round-btn ${saved ? "on" : ""}`} style={{ background: "var(--surface-2)" }} aria-pressed={saved} aria-label={tr(lang, "Зберегти", "Save")} onClick={toggleSave}><Heart className="ico sm" /></button>
        {bookButton(false)}
      </div>

      {booking && (
        <Modal title={tr(lang, "Бронювання", "Booking")} onClose={() => !busy && setBooking(false)}>
          <div className="stack lg">
            <div>
              <div className="strong">{event.title}</div>
              <div className="muted small">{longDate(event.starts_at, lang, event.timezone)} · {timeOf(event.starts_at, event.timezone)}</div>
            </div>
            <div className="row spread">
              <div>
                <div className="strong">{tr(lang, "Кількість місць", "Places")}</div>
                <div className="muted xs">{tr(lang, `до ${canMore} · кожне місце — окремий QR`, `up to ${canMore} · one QR per place`)}</div>
              </div>
              <div className="stepper" role="group" aria-label={tr(lang, "Кількість місць", "Places")}>
                <button onClick={() => setQty((q) => Math.max(1, q - 1))} disabled={qty <= 1} aria-label={tr(lang, "Менше", "Fewer")}><Minus className="ico sm" /></button>
                <output aria-live="polite">{qty}</output>
                <button onClick={() => setQty((q) => Math.min(canMore, q + 1))} disabled={qty >= canMore} aria-label={tr(lang, "Більше", "More")}><Plus className="ico sm" /></button>
              </div>
            </div>
            {needDob && (
              <div className="field">
                <label htmlFor="dob">{tr(lang, `Дата народження (подія ${ageMin}+)`, `Date of birth (${ageMin}+ event)`)}</label>
                <input id="dob" className="input" type="date" value={dob} max={new Date().toISOString().slice(0, 10)} onChange={(e) => setDob(e.target.value)} />
                <span className="hint">{tr(lang, "Збережемо в профілі, щоб не питати знову.", "We'll keep it in your profile.")}</span>
              </div>
            )}
            {error && <div className="alert error">{error}</div>}
            <button className="btn block lg" onClick={book} disabled={busy || qty < 1 || qty > canMore || (needDob && !dob)}>
              {busy ? tr(lang, "Бронюємо…", "Booking…") : tr(lang, `Забронювати ${placesLabel(qty, lang)}`, `Book ${qty} ${qty === 1 ? "place" : "places"}`)}
            </button>
            <p className="subtle xs" style={{ textAlign: "center" }}>{tr(lang, "Безкоштовно. Організатор побачить лише ваше імʼя і кількість місць.", "Free. The organiser sees only your name and the number of places.")}</p>
          </div>
        </Modal>
      )}

      {reportOpen && (
        <Modal title={reportOpen.type === "review" ? tr(lang, "Скарга на відгук", "Report review") : tr(lang, "Скарга на подію", "Report event")} onClose={() => setReportOpen(null)}>
          <form className="form" onSubmit={(e) => { e.preventDefault(); submitReport(); }}>
            <div className="field">
              <label htmlFor="reason">{tr(lang, "Що не так?", "What's wrong?")}</label>
              <textarea id="reason" className="input" rows={3} value={reportReason} onChange={(e) => setReportReason(e.target.value)} placeholder={tr(lang, "Спам, шахрайство, образливий зміст…", "Spam, fraud, offensive content…")} required />
            </div>
            <button className="btn block" disabled={!reportReason.trim() || busy}>{tr(lang, "Надіслати скаргу", "Send report")}</button>
          </form>
        </Modal>
      )}
      <Toast msg={toast} />
    </main>
  );
}
