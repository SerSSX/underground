"use client";

import Link from "next/link";
import { Suspense, useEffect, useMemo, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import QRCode from "qrcode";
import { myTickets, returnTicket, giftTicket, revokeGift, ticketQr, getToken, errorMessage, mapsUrl, isOver, type TicketDto } from "../../../lib/api";
import { useLang, tr } from "../../../lib/i18n";
import { longDate, timeOf, placesLabel, shortWhen } from "../../../lib/ui";
import Modal from "../../components/Modal";
import Poster from "../../components/Poster";
import { Toast, useToast } from "../../components/Toast";
import { QrCode, Navigation, Gift, X, Ticket, Compass, CircleCheck, ChevronLeft, ChevronRight } from "../../components/Icon";

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

function TicketsInner() {
  const router = useRouter();
  const params = useSearchParams();
  const { lang } = useLang();
  // The success banner is shown once, right after a booking; the numbers come from the data, not the URL.
  const [bookedEvent] = useState(params.get("booked"));
  useEffect(() => { if (params.get("booked")) router.replace("/me/tickets"); }, [params, router]);
  const [tickets, setTickets] = useState<TicketDto[] | null>(null);
  const [tab, setTab] = useState<"upcoming" | "past">("upcoming");
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
  }, [router]);

  const groups = useMemo(() => {
    const all = group((tickets ?? []).filter((t) => t.status !== "returned"));
    // An event stays here until it is over (its end, or 12 h after the start), so a late guest still has the QR.
    const upcoming = all.filter((g) => !isOver(g.startsAt, g.endsAt) && g.tickets.some((t) => t.status === "active" || t.status === "pending_claim"));
    const past = all.filter((g) => !upcoming.includes(g));
    upcoming.sort((a, b) => (a.startsAt ?? "").localeCompare(b.startsAt ?? ""));
    past.sort((a, b) => (b.startsAt ?? "").localeCompare(a.startsAt ?? ""));
    return { upcoming, past };
  }, [tickets]);

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
      showToast(tr(lang, `Скасовано: ${placesLabel(toCancel.length, lang)}`, `Cancelled ${toCancel.length}`));
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

  const list = tab === "upcoming" ? groups.upcoming : groups.past;
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
                {bookedGroup.title} · {placesLabel(bookedGroup.tickets.filter((t) => t.status === "active").length, lang)}. {tr(lang, "На вході покажіть QR — для кожного місця свій.", "Show the QR at the door — one per place.")}
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
        <button role="tab" aria-selected={tab === "upcoming"} className={tab === "upcoming" ? "active" : ""} onClick={() => setTab("upcoming")}>{tr(lang, "Майбутні", "Upcoming")} <span className="count">{groups.upcoming.length}</span></button>
        <button role="tab" aria-selected={tab === "past"} className={tab === "past" ? "active" : ""} onClick={() => setTab("past")}>{tr(lang, "Минулі", "Past")} <span className="count">{groups.past.length}</span></button>
      </div>

      {error && <div className="alert error" style={{ marginBottom: 16 }}>{error}</div>}

      {tickets === null ? (
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
                <Link href={`/events/${g.eventId}`} className="thumb" aria-label={g.title}><Poster src={g.poster} title={g.title} category={null} /></Link>
                <div style={{ minWidth: 0 }}>
                  <Link href={`/events/${g.eventId}`}><h3>{g.title}</h3></Link>
                  <div className="muted small" style={{ marginTop: 2 }}>{shortWhen(g.startsAt, lang, g.tz)}</div>
                  <div className="muted small">{[g.venue, g.city].filter(Boolean).join(", ")}</div>
                  <div className="row gap wrap" style={{ marginTop: 8 }}>
                    {act.length > 0 && <span className="badge ok">{placesLabel(act.length, lang)}</span>}
                    {used.length > 0 && <span className="badge">{tr(lang, `пройшли: ${used.length}`, `checked in: ${used.length}`)}</span>}
                    {pending.length > 0 && <span className="badge warn">{tr(lang, `передається: ${pending.length}`, `being transferred: ${pending.length}`)}</span>}
                  </div>
                </div>
                <div className="actions row gap wrap" style={{ justifyContent: "flex-end" }}>
                  {act.length > 0 && tab === "upcoming" && (
                    <>
                      <button className="btn sm" onClick={() => { setQrIdx(0); setQrFor(g); }}><QrCode className="ico sm" />QR</button>
                      {maps && <a className="btn ghost sm icon" href={maps} target="_blank" rel="noreferrer" aria-label={tr(lang, "Маршрут", "Directions")}><Navigation className="ico sm" /></a>}
                      <button className="btn ghost sm icon" aria-label={tr(lang, "Передати місце", "Transfer a place")} title={tr(lang, "Передати місце", "Transfer a place")} onClick={() => { setGiftFor(act[0]); setGiftEmail(""); setGiftLink(null); }}><Gift className="ico sm" /></button>
                      <button className="btn danger-ghost sm" onClick={() => { setCancelN(act.length); setCancelFor(g); }}>{tr(lang, "Скасувати", "Cancel")}</button>
                    </>
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
                  <span className="strong">{tr(lang, `Місце ${qrIdx + 1} з ${active.length}`, `Place ${qrIdx + 1} of ${active.length}`)}</span>
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
              <button className="btn danger grow" disabled={busy} onClick={doCancel}>{busy ? "…" : tr(lang, `Скасувати ${placesLabel(cancelN, lang)}`, `Cancel ${cancelN}`)}</button>
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
