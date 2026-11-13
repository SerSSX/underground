"use client";

import Link from "next/link";
import { Suspense, useEffect, useMemo, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import QRCode from "qrcode";
import {
  myTickets, returnTicket, giftTicket, revokeGift, ticketQr, getToken, errorMessage, mapsUrl, isOver, mySaves, unsaveEvent, getEvent,
  type TicketDto, type EventDto,
} from "../../../lib/api";
import { splitTicketGroups } from "../../../lib/event-time.mjs";
import { useSession } from "../../../lib/session";
import EventCard from "../../components/EventCard";
import { useLang, tr, tf, tp } from "../../../lib/i18n";
import { longDate, timeOf, placesLabel, shortWhen } from "../../../lib/ui";
import Modal from "../../components/Modal";
import Poster from "../../components/Poster";
import { Toast, useToast } from "../../components/Toast";
import { QrCode, Navigation, Gift, X, Ticket, Compass, CircleCheck, ChevronLeft, ChevronRight, Heart, MessageCircle } from "../../components/Icon";

type Group = { eventId: string; title: string; startsAt: string | null; endsAt: string | null; tz: string | null; city: string | null; venue: string | null; poster: string | null; tickets: TicketDto[] };

/** Tickets grouped by event: one card per event with all its places. */
function group(list: TicketDto[]): Group[] {
  const map = new Map<string, Group>();
  for (const t of list) {
    const g = map.get(t.event_id) ?? { eventId: t.event_id, title: t.event_title, startsAt: t.event_starts_at, endsAt: t.event_ends_at ?? null, tz: t.event_timezone ?? null, city: t.event_city, venue: t.event_venue_address ?? null, poster: t.event_poster_url ?? null, tickets: [] };
    g.tickets.push(t);
    map.set(t.event_id, g);
  }
  return [...map.values()];
}

type Tab = "upcoming" | "past" | "saved";
/** Upcoming events whose organiser is looked up for «Написати організатору» (tickets do not carry it). */
const MAX_ORGANISER_LOOKUPS = 12;

function TicketsInner() {
  const router = useRouter();
  const params = useSearchParams();
  const { lang } = useLang();
  const { me } = useSession();
  // The success banner is shown once, right after a booking; the numbers come from the data, not the URL.
  const [bookedEvent] = useState(params.get("booked"));
  useEffect(() => { if (params.get("booked")) router.replace("/me/tickets"); }, [params, router]);
  const [tickets, setTickets] = useState<TicketDto[] | null>(null);
  // ?tab=saved opens the saved events (profile menu, account page).
  const [tab, setTabState] = useState<Tab>(params.get("tab") === "saved" ? "saved" : "upcoming");
  const setTab = (t: Tab) => {
    setTabState(t);
    window.history.replaceState(null, "", t === "saved" ? "/me/tickets?tab=saved" : "/me/tickets");
  };
  // The profile menu's "Saved" link while this page is already open changes only the query.
  useEffect(() => { if (params.get("tab") === "saved") setTabState("saved"); }, [params]);
  const [saves, setSaves] = useState<EventDto[] | null>(null);
  // Event id → its organiser's id, for the upcoming cards' «Написати організатору».
  const [organisers, setOrganisers] = useState<Record<string, string | null>>({});
  const [error, setError] = useState<string | null>(null);
  const [qrFor, setQrFor] = useState<Group | null>(null);
  const [qrIdx, setQrIdx] = useState(0);
  const [qrImg, setQrImg] = useState<string | null>(null);
  const [cancelFor, setCancelFor] = useState<Group | null>(null);
  const [cancelN, setCancelN] = useState(1);
  const [giftFor, setGiftFor] = useState<TicketDto | null>(null);
  const [giftEmail, setGiftEmail] = useState("");
  const [giftLink, setGiftLink] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [toast, showToast] = useToast();

  async function load() {
    setTickets(await myTickets());
  }
  useEffect(() => {
    if (!getToken()) {
      router.replace("/login?next=/me/tickets");
      return;
    }
    load().catch((e) => { setError(errorMessage(e)); setTickets([]); });
    mySaves().then(setSaves).catch(() => setSaves([]));
  }, [router]);

  // An event stays under Upcoming until it is over (its end, or 12 h after the start), so a late guest still has
  // the QR — and a guest already scanned in still finds tonight's event there (lib/event-time.mjs, TASK-305).
  const groups = useMemo(() => splitTicketGroups(group((tickets ?? []).filter((t) => t.status !== "returned"))), [tickets]);

  // Saved events: still to come first (soonest first), then the ones that are over.
  const savedList = useMemo(() => {
    const l = [...(saves ?? [])];
    const over = (e: EventDto) => isOver(e.starts_at, e.ends_at);
    return l.sort((a, b) => Number(over(a)) - Number(over(b)) || (a.starts_at ?? "9").localeCompare(b.starts_at ?? "9"));
  }, [saves]);

  // The organiser of each upcoming event (bounded): read once per event, never for the user's own event.
  const upcomingIds = groups.upcoming.slice(0, MAX_ORGANISER_LOOKUPS).map((g) => g.eventId).join(",");
  useEffect(() => {
    const ids = upcomingIds ? upcomingIds.split(",") : [];
    const missing = ids.filter((id) => !(id in organisers));
    if (missing.length === 0) return;
    let stop = false;
    Promise.all(missing.map((id) => getEvent(id).then((e) => [id, e.organizer_id] as const).catch(() => [id, null] as const)))
      .then((pairs) => { if (!stop) setOrganisers((o) => ({ ...o, ...Object.fromEntries(pairs) })); });
    return () => { stop = true; };
    // `organisers` is left out of the deps on purpose: it only filters out ids already looked up.
  }, [upcomingIds]);

  async function unsave(id: string) {
    setSaves((l) => (l ?? []).filter((e) => e.id !== id));
    try {
      await unsaveEvent(id);
      showToast(tr(lang, "Прибрано зі збережених", "Removed from saved"));
    } catch (e) {
      setError(errorMessage(e));
      // Show what the server really holds (another unsave may have gone through meanwhile).
      mySaves().then(setSaves).catch(() => {});
    }
  }

  // The door QR is a signed token that lives about a minute: fetch it on open and rotate it while shown.
  const active = qrFor?.tickets.filter((t) => t.status === "active") ?? [];
  const current = active[qrIdx];
  useEffect(() => {
    if (!current) { setQrImg(null); return; }
    let stop = false;
    const draw = async () => {
      try {
        const { token } = await ticketQr(current.id);
        const url = await QRCode.toDataURL(token, { margin: 0, width: 560, errorCorrectionLevel: "M" });
        if (!stop) setQrImg(url);
      } catch { /* a later tick retries */ }
    };
    setQrImg(null);
    void draw();
    const id = setInterval(draw, 45_000);
    return () => { stop = true; clearInterval(id); };
  }, [current?.id]);

  async function doCancel() {
    if (!cancelFor) return;
    setBusy(true);
    setError(null);
    try {
      const toCancel = cancelFor.tickets.filter((t) => t.status === "active").slice(0, cancelN);
      for (const t of toCancel) await returnTicket(t.id);
      await load();
      showToast(tp(lang, toCancel.length, { one: "Скасовано: {n} місце", few: "Скасовано: {n} місця", many: "Скасовано: {n} місць" }, { one: "Cancelled {n}", other: "Cancelled {n}" }));
      setCancelFor(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function doGift() {
    if (!giftFor || !giftEmail.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const res = await giftTicket(giftFor.id, giftEmail.trim());
      setGiftLink(`${window.location.origin}/g/${res.claim_token}`);
      await load();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function doRevoke(t: TicketDto) {
    setBusy(true);
    try {
      await revokeGift(t.id);
      await load();
      showToast(tr(lang, "Передачу скасовано", "Transfer cancelled"));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const list = tab === "upcoming" ? groups.upcoming : tab === "past" ? groups.past : [];
  const bookedGroup = bookedEvent ? groups.upcoming.find((g) => g.eventId === bookedEvent) : null;

  return (
    <main className="container narrow">
      <div className="page-head"><h1>{tr(lang, "Мої квитки", "My tickets")}</h1></div>

      {bookedGroup && (
        <div className="card" style={{ marginBottom: 20, borderColor: "rgba(52,199,123,.35)", background: "linear-gradient(180deg, rgba(52,199,123,.10), transparent), var(--surface)" }}>
          <div className="row gap" style={{ alignItems: "flex-start" }}>
            <CircleCheck className="ico lg" style={{ color: "var(--ok-fg)", marginTop: 2 }} />
            <div className="grow">
              <h3>{tr(lang, "Готово! Місця заброньовано", "Done! Your places are booked")}</h3>
              <p className="muted small" style={{ marginTop: 4 }}>
                {bookedGroup.title} · {placesLabel(bookedGroup.tickets.filter((t) => t.status === "active").length, lang)}. {bookedGroup.tickets.filter((t) => t.status === "active").length > 1 ? tr(lang, "На вході покажіть QR — для кожного місця свій.", "Show the QR at the door — one per place.") : tr(lang, "На вході покажіть QR.", "Show the QR at the door.")}
              </p>
              <div className="row gap wrap" style={{ marginTop: 12 }}>
                <button className="btn sm" onClick={() => { setQrIdx(0); setQrFor(bookedGroup); }}><QrCode className="ico sm" />{tr(lang, "Показати QR", "Show QR")}</button>
                <Link className="btn ghost sm" href={`/events/${bookedGroup.eventId}`}>{tr(lang, "До події", "To the event")}</Link>
              </div>
            </div>
          </div>
        </div>
      )}

      <div className="tabs" role="tablist">
        <button role="tab" aria-selected={tab === "upcoming"} className={tab === "upcoming" ? "active" : ""} onClick={() => setTab("upcoming")}>{tr(lang, "Майбутні", "Upcoming")} {tickets && <span className="count">{groups.upcoming.length}</span>}</button>
        <button role="tab" aria-selected={tab === "past"} className={tab === "past" ? "active" : ""} onClick={() => setTab("past")}>{tr(lang, "Минулі", "Past")} {tickets && <span className="count">{groups.past.length}</span>}</button>
        <button role="tab" aria-selected={tab === "saved"} className={tab === "saved" ? "active" : ""} onClick={() => setTab("saved")}><Heart className="ico sm" aria-hidden />{tr(lang, "Збережені", "Saved")} {saves && <span className="count">{saves.length}</span>}</button>
      </div>

      {error && <div className="alert error" style={{ marginBottom: 16 }}>{error}</div>}

      {tab === "saved" ? (
        saves === null ? (
          <div className="grid">{[0, 1, 2].map((i) => <div key={i} className="skeleton" style={{ height: 260 }} />)}</div>
        ) : savedList.length === 0 ? (
          <div className="empty">
            <div className="ei"><Heart className="ico lg" /></div>
            <h3>{tr(lang, "Поки нічого не збережено", "Nothing saved yet")}</h3>
            <p>{tr(lang, "Натисніть сердечко на події, щоб не загубити її — вона зʼявиться тут.", "Tap the heart on an event to keep it — it shows up here.")}</p>
            <Link href="/" className="btn"><Compass className="ico sm" />{tr(lang, "До афіші", "Browse events")}</Link>
          </div>
        ) : (
          // The heart here only removes: an event leaves the list as soon as it is unsaved.
          <div className="grid">{savedList.map((e) => <EventCard key={e.id} event={e} saved onToggleSave={(id) => void unsave(id)} />)}</div>
        )
      ) : tickets === null ? (
        <div className="stack">{[0, 1, 2].map((i) => <div key={i} className="skeleton" style={{ height: 120 }} />)}</div>
      ) : list.length === 0 ? (
        <div className="empty">
          <div className="ei"><Ticket className="ico lg" /></div>
          <h3>{tab === "upcoming" ? tr(lang, "Поки немає квитків", "No tickets yet") : tr(lang, "Тут зʼявляться минулі події", "Past events will show up here")}</h3>
          <p>{tr(lang, "Знайдіть подію в афіші й забронюйте місце — квиток з QR зʼявиться тут.", "Find an event and book a place — the QR ticket appears here.")}</p>
          <Link href="/" className="btn"><Compass className="ico sm" />{tr(lang, "До афіші", "Browse events")}</Link>
        </div>
      ) : (
        <div className="stack">
          {list.map((g) => {
            const act = g.tickets.filter((t) => t.status === "active");
            const used = g.tickets.filter((t) => t.status === "used");
            const pending = g.tickets.filter((t) => t.status === "pending_claim");
            const maps = mapsUrl(g.venue, g.city);
            return (
              <article key={g.eventId} className="tcard">
                <Link href={`/events/${g.eventId}`} className="thumb" aria-label={g.title}><Poster src={g.poster} title={g.title} category={null} variant="thumb" /></Link>
                <div style={{ minWidth: 0 }}>
                  <Link href={`/events/${g.eventId}`}><h3>{g.title}</h3></Link>
                  <div className="muted small" style={{ marginTop: 2 }}>{shortWhen(g.startsAt, lang, g.tz)}</div>
                  <div className="muted small">{[g.venue, g.city].filter(Boolean).join(", ")}</div>
                  <div className="row gap wrap" style={{ marginTop: 8 }}>
                    {act.length > 0 && <span className="badge ok">{placesLabel(act.length, lang)}</span>}
                    {used.length > 0 && <span className="badge">{tf(lang, "пройшли: {n}", "checked in: {n}", { n: used.length })}</span>}
                    {pending.length > 0 && <span className="badge warn">{tf(lang, "передається: {n}", "being transferred: {n}", { n: pending.length })}</span>}
                  </div>
                </div>
                <div className="actions row gap wrap" style={{ justifyContent: "flex-end" }}>
                  {act.length > 0 && tab === "upcoming" && (
                    <>
                      <button className="btn sm" onClick={() => { setQrIdx(0); setQrFor(g); }}><QrCode className="ico sm" />QR</button>
                      {maps && <a className="btn ghost sm" href={maps} target="_blank" rel="noreferrer"><Navigation className="ico sm" />{tr(lang, "Маршрут", "Directions")}</a>}
                      <button className="btn ghost sm" onClick={() => { setGiftFor(act[0]); setGiftEmail(""); setGiftLink(null); }}><Gift className="ico sm" />{tr(lang, "Передати", "Give")}</button>
                      <button className="btn danger-ghost sm" onClick={() => { setCancelN(1); setCancelFor(g); }}>{tr(lang, "Скасувати", "Cancel")}</button>
                    </>
                  )}
                  {tab === "upcoming" && organisers[g.eventId] && organisers[g.eventId] !== me?.id && (
                    <Link className="btn ghost sm" href={`/inbox?to=${organisers[g.eventId]}&about=${g.eventId}`}><MessageCircle className="ico sm" />{tr(lang, "Написати організатору", "Message the organiser")}</Link>
                  )}
                  {pending.map((t) => (
                    <button key={t.id} className="btn ghost sm" disabled={busy} onClick={() => doRevoke(t)}><X className="ico sm" />{tr(lang, "Скасувати передачу", "Cancel transfer")}</button>
                  ))}
                </div>
              </article>
            );
          })}
        </div>
      )}

      {qrFor && (
        <Modal title={qrFor.title} onClose={() => setQrFor(null)}>
          {active.length === 0 ? (
            <p className="muted">{tr(lang, "Активних квитків немає.", "No active tickets.")}</p>
          ) : (
            <div className="stack lg" style={{ alignItems: "center", textAlign: "center" }}>
              <div className="muted small">{longDate(qrFor.startsAt, lang, qrFor.tz)} · {timeOf(qrFor.startsAt, qrFor.tz)}</div>
              <div className="qr-box">
                {qrImg ? <img src={qrImg} alt={tr(lang, "QR-код квитка", "Ticket QR code")} /> : <div className="skeleton" style={{ width: "100%", height: "100%" }} />}
              </div>
              {active.length > 1 && (
                <div className="row gap">
                  <button className="btn ghost icon sm" disabled={qrIdx === 0} onClick={() => setQrIdx((i) => i - 1)} aria-label={tr(lang, "Попереднє місце", "Previous place")}><ChevronLeft className="ico sm" /></button>
                  <span className="strong">{tf(lang, "Місце {i} з {total}", "Place {i} of {total}", { i: qrIdx + 1, total: active.length })}</span>
                  <button className="btn ghost icon sm" disabled={qrIdx >= active.length - 1} onClick={() => setQrIdx((i) => i + 1)} aria-label={tr(lang, "Наступне місце", "Next place")}><ChevronRight className="ico sm" /></button>
                </div>
              )}
              <p className="subtle xs">{tr(lang, "Код оновлюється сам кожну хвилину — скриншот не спрацює. Зробіть екран яскравішим.", "The code refreshes every minute — a screenshot won't work. Turn up the brightness.")}</p>
            </div>
          )}
        </Modal>
      )}

      {cancelFor && (
        <Modal title={tr(lang, "Скасувати бронь?", "Cancel booking?")} onClose={() => !busy && setCancelFor(null)}>
          <div className="stack lg">
            <p className="muted">{tr(lang, "Місця знову стануть вільними для інших. Забронювати ще раз можна, поки є місця.", "The places go back to others. You can book again while places are left.")}</p>
            {cancelFor.tickets.filter((t) => t.status === "active").length > 1 && (
              <div className="row spread">
                <span className="strong">{tr(lang, "Скільки місць скасувати", "How many to cancel")}</span>
                <div className="seg">
                  {cancelFor.tickets.filter((t) => t.status === "active").map((_, i) => (
                    <button key={i} className={cancelN === i + 1 ? "active" : ""} onClick={() => setCancelN(i + 1)}>{i + 1}</button>
                  ))}
                </div>
              </div>
            )}
            {error && <div className="alert error">{error}</div>}
            <div className="row gap">
              <button className="btn ghost grow" disabled={busy} onClick={() => setCancelFor(null)}>{tr(lang, "Залишити", "Keep")}</button>
              <button className="btn danger grow" disabled={busy} onClick={doCancel}>{busy ? "…" : tp(lang, cancelN, { one: "Скасувати {n} місце", few: "Скасувати {n} місця", many: "Скасувати {n} місць" }, { one: "Cancel {n}", other: "Cancel {n}" })}</button>
            </div>
          </div>
        </Modal>
      )}

      {giftFor && (
        <Modal title={tr(lang, "Передати місце другу", "Give a place to a friend")} onClose={() => !busy && setGiftFor(null)}>
          {giftLink ? (
            <div className="stack lg">
              <div className="alert ok"><CircleCheck className="ico" />{tr(lang, "Готово. Надішліть другові це посилання — він забере місце у свій акаунт.", "Done. Send this link to your friend to claim the place.")}</div>
              <input className="input" readOnly value={giftLink} onFocus={(e) => e.currentTarget.select()} aria-label={tr(lang, "Посилання для друга", "Link for your friend")} />
              <button className="btn block" onClick={() => { void navigator.clipboard.writeText(giftLink); showToast(tr(lang, "Скопійовано", "Copied")); }}>{tr(lang, "Скопіювати посилання", "Copy link")}</button>
            </div>
          ) : (
            <form className="form" onSubmit={(e) => { e.preventDefault(); void doGift(); }}>
              <p className="muted small">{tr(lang, "Одне місце перейде другові: ваш QR для нього перестане діяти, у друга зʼявиться свій.", "One place moves to your friend: your QR for it stops working, they get their own.")}</p>
              <div className="field">
                <label htmlFor="gift-email">{tr(lang, "Email друга", "Friend's email")}</label>
                <input id="gift-email" className="input" type="email" autoComplete="email" value={giftEmail} onChange={(e) => setGiftEmail(e.target.value)} required />
              </div>
              {error && <div className="alert error">{error}</div>}
              <button className="btn block" disabled={busy || !giftEmail.trim()}>{busy ? "…" : tr(lang, "Створити посилання", "Create link")}</button>
            </form>
          )}
        </Modal>
      )}
      <Toast msg={toast} />
    </main>
  );
}

export default function MyTicketsPage() {
  return (
    <Suspense fallback={<main className="container narrow"><div className="skeleton" style={{ height: 120 }} /></main>}>
      <TicketsInner />
    </Suspense>
  );
}
