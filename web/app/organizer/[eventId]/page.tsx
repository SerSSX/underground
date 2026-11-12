"use client";

import Link from "next/link";
import { Suspense, useCallback, useEffect, useMemo, useState } from "react";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import {
  myEvents, myStaffEvents, eventGuests, inviteStaff, updateEvent, cancelEvent, getToken, errorMessage, isOver,
  listStaff, revokeStaffInvite, removeStaff,
  type EventDto, type GuestDto, type StaffList,
} from "../../../lib/api";
import { useLang, tr } from "../../../lib/i18n";
import { useSession } from "../../../lib/session";
import { shortWhen, initials, placesLabel, plural } from "../../../lib/ui";
import Modal from "../../components/Modal";
import Poster from "../../components/Poster";
import Scanner from "../../components/Scanner";
import { Toast, useToast } from "../../components/Toast";
import { Users, ScanLine, UserPlus, Pencil, Share2, ExternalLink, Search, ChevronLeft, Copy, Lock, X, Download } from "../../components/Icon";
import { toCsv } from "../../../lib/csv.mjs";

type Tab = "guests" | "scan" | "staff";

function ManageInner() {
  const { eventId } = useParams<{ eventId: string }>();
  const router = useRouter();
  const params = useSearchParams();
  const { lang } = useLang();
  const { me } = useSession();
  const [event, setEvent] = useState<EventDto | null>(null);
  const [owner, setOwner] = useState(false);
  const [guests, setGuests] = useState<GuestDto[] | null>(null);
  const [tab, setTab] = useState<Tab>((params.get("tab") as Tab) || "guests");
  const [q, setQ] = useState("");
  const [invite, setInvite] = useState<string | null>(null);
  const [staff, setStaff] = useState<StaffList | null>(null);
  const [confirm, setConfirm] = useState<null | "cancel">(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [toast, showToast] = useToast();

  useEffect(() => {
    if (!getToken()) {
      router.replace(`/login?next=${encodeURIComponent(`/organizer/${eventId}`)}`);
      return;
    }
    (async () => {
      const mine = await myEvents().catch(() => [] as EventDto[]);
      const own = mine.find((e) => e.id === eventId);
      if (own) {
        setEvent(own);
        setOwner(true);
        eventGuests(eventId).then(setGuests).catch((e) => { setError(errorMessage(e)); setGuests([]); });
        listStaff(eventId).then(setStaff).catch(() => setStaff({ staff: [], invites: [] }));
        return;
      }
      const staff = await myStaffEvents().catch(() => [] as EventDto[]);
      const s = staff.find((e) => e.id === eventId);
      if (s) {
        setEvent(s);
        setTab("scan");
      } else {
        setNotFound(true);
      }
    })();
  }, [eventId, router]);

  const shown = useMemo(() => {
    const n = q.trim().toLowerCase();
    return (guests ?? []).filter((g) => !n || g.name.toLowerCase().includes(n));
  }, [guests, q]);
  const totals = useMemo(() => {
    const g = guests ?? [];
    return { people: g.length, places: g.reduce((a, x) => a + x.places, 0), inside: g.reduce((a, x) => a + x.checked_in, 0) };
  }, [guests]);

  const refreshGuests = useCallback(() => { eventGuests(eventId).then(setGuests).catch(() => {}); }, [eventId]);

  async function makeInvite() {
    setBusy(true);
    setError(null);
    try {
      const r = await inviteStaff(eventId);
      setInvite(`${window.location.origin}/staff/accept#token=${encodeURIComponent(r.invite_token)}`);
      listStaff(eventId).then(setStaff).catch(() => {});
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function revoke(inviteId: string) {
    setBusy(true);
    try {
      await revokeStaffInvite(eventId, inviteId);
      setInvite(null);
      setStaff(await listStaff(eventId));
      showToast(tr(lang, "Запрошення відкликано", "Invite revoked"));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function dropStaff(userId: string) {
    setBusy(true);
    try {
      await removeStaff(eventId, userId);
      setStaff(await listStaff(eventId));
      showToast(tr(lang, "Доступ забрано", "Access removed"));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function toggleBooking() {
    if (!event) return;
    setBusy(true);
    try {
      const upd = await updateEvent(event.id, { sales_closed: !event.sales_closed });
      setEvent({ ...event, ...upd });
      showToast(upd.sales_closed ? tr(lang, "Бронювання закрито", "Booking closed") : tr(lang, "Бронювання відкрито", "Booking open"));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function doCancel() {
    if (!event) return;
    setBusy(true);
    try {
      await cancelEvent(event.id);
      router.push("/organizer");
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  }

  /** Guest list as a spreadsheet: name, places, checked in — nothing else (no email exists here). */
  function exportCsv() {
    if (!event || !guests) return;
    const rows = [[tr(lang, "Імʼя", "Name"), tr(lang, "Місць", "Places"), tr(lang, "Прийшли", "Checked in")], ...guests.map((g) => [g.name, g.places, g.checked_in])];
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob(["\ufeff" + toCsv(rows)], { type: "text/csv;charset=utf-8" }));
    a.download = `guests-${event.id.slice(0, 8)}.csv`;
    a.click();
  }

  function share() {
    if (!event) return;
    const url = `${window.location.origin}/events/${event.id}`;
    if (navigator.share) void navigator.share({ title: event.title, url }).catch(() => {});
    else void navigator.clipboard.writeText(url).then(() => showToast(tr(lang, "Посилання скопійовано", "Link copied")));
  }

  if (notFound) {
    return (
      <main className="container narrow">
        <div className="empty" style={{ marginTop: 24 }}>
          <div className="ei"><Lock className="ico lg" /></div>
          <h3>{tr(lang, "Немає доступу", "No access")}</h3>
          <p>{tr(lang, "Цю подію може відкрити лише її організатор або запрошений ним персонал.", "Only the organiser or staff they invited can open this.")}</p>
          <Link className="btn" href="/organizer">{tr(lang, "До моїх подій", "My events")}</Link>
        </div>
      </main>
    );
  }
  if (!event) return <main className="container"><div className="skeleton" style={{ height: 160 }} /></main>;

  const total = event.tickets_total ?? 0;
  const booked = total > 0 ? total - event.tickets_left : totals.places;
  const live = event.status === "published" && !isOver(event.starts_at, event.ends_at);

  return (
    <main className="container">
      <Link href="/organizer" className="btn quiet sm" style={{ marginLeft: -12, marginBottom: 8 }}><ChevronLeft className="ico sm" />{tr(lang, "Мої події", "My events")}</Link>

      <div className="orow" style={{ marginBottom: 20 }}>
        <div className="thumb"><Poster src={event.poster_url} title={event.title} category={event.category} /></div>
        <div style={{ minWidth: 0 }}>
          <h1 style={{ fontSize: 24 }}>{event.title}</h1>
          <div className="muted small" style={{ marginTop: 4 }}>{shortWhen(event.starts_at, lang, event.timezone)}{event.city ? ` · ${event.city}` : ""}</div>
          {owner && (
            <div className="row gap wrap" style={{ marginTop: 8 }}>
              <span className="badge accent">{tr(lang, `Заброньовано ${booked}${total ? ` з ${total}` : ""}`, `${booked}${total ? ` of ${total}` : ""} booked`)}</span>
              <span className="badge ok">{tr(lang, `Прийшли ${totals.inside}`, `${totals.inside} checked in`)}</span>
              {event.sales_closed && <span className="badge warn">{tr(lang, "Бронювання закрите", "Booking closed")}</span>}
            </div>
          )}
        </div>
        {owner && (
          <div className="actions row gap wrap" style={{ justifyContent: "flex-end" }}>
            {event.status === "published" && <Link className="btn ghost sm" href={`/events/${event.id}`}><ExternalLink className="ico sm" />{tr(lang, "Сторінка", "Page")}</Link>}
            <Link className="btn ghost sm" href={`/organizer/${event.id}/edit`}><Pencil className="ico sm" />{tr(lang, "Редагувати", "Edit")}</Link>
            {event.status === "published" && <button className="btn ghost sm icon" onClick={share} aria-label={tr(lang, "Поділитися", "Share")}><Share2 className="ico sm" /></button>}
          </div>
        )}
      </div>

      <div className="tabs" role="tablist">
        {owner && <button role="tab" aria-selected={tab === "guests"} className={tab === "guests" ? "active" : ""} onClick={() => setTab("guests")}><Users className="ico sm" />{tr(lang, "Гості", "Guests")} <span className="count">{totals.people}</span></button>}
        <button role="tab" aria-selected={tab === "scan"} className={tab === "scan" ? "active" : ""} onClick={() => setTab("scan")}><ScanLine className="ico sm" />{tr(lang, "Сканер", "Scanner")}</button>
        {owner && <button role="tab" aria-selected={tab === "staff"} className={tab === "staff" ? "active" : ""} onClick={() => setTab("staff")}><UserPlus className="ico sm" />{tr(lang, "Персонал", "Staff")}</button>}
      </div>

      {error && <div className="alert error" style={{ marginBottom: 14 }}>{error}</div>}

      {tab === "guests" && owner && (
        guests === null ? <div className="skeleton" style={{ height: 240 }} /> : guests.length === 0 ? (
          <div className="empty">
            <div className="ei"><Users className="ico lg" /></div>
            <h3>{tr(lang, "Поки ніхто не забронював", "No bookings yet")}</h3>
            <p>{tr(lang, "Поділіться посиланням на подію — гості зʼявляться тут з іменем і кількістю місць.", "Share the event link — guests show up here with their name and number of places.")}</p>
            {event.status === "published" && <button className="btn" onClick={share}><Share2 className="ico sm" />{tr(lang, "Поділитися", "Share")}</button>}
          </div>
        ) : (
          <div className="card" style={{ padding: 16 }}>
            <div className="row spread wrap" style={{ gap: 12, marginBottom: 6 }}>
              <span className="muted small">{tr(lang, `${totals.people} ${plural(totals.people, ["гість", "гості", "гостей"])} · ${placesLabel(totals.places, lang)}`, `${totals.people} guests · ${totals.places} places`)}</span>
              <div className="row gap wrap">
              <button className="btn ghost sm" onClick={exportCsv}><Download className="ico sm" />CSV</button>
              <label className="input-icon" style={{ minWidth: 220 }}>
                <span className="sr-only">{tr(lang, "Пошук гостя", "Find a guest")}</span>
                <Search className="ico" />
                <input className="input" style={{ minHeight: 40 }} value={q} onChange={(e) => setQ(e.target.value)} placeholder={tr(lang, "Пошук за імʼям", "Search by name")} />
              </label>
              </div>
            </div>
            {shown.map((g, i) => (
              <div key={`${g.name}-${i}`} className="guest">
                <span className="avatar sm">{initials(g.name)}</span>
                <span className="grow strong">{g.name}</span>
                <span className="badge">{placesLabel(g.places, lang)}</span>
                {g.checked_in > 0 && <span className="badge ok">{g.checked_in === g.places ? tr(lang, "прийшли", "in") : tr(lang, `прийшли ${g.checked_in}`, `${g.checked_in} in`)}</span>}
              </div>
            ))}
          </div>
        )
      )}

      {tab === "scan" && (live ? <Scanner eventId={event.id} onAdmitted={owner ? refreshGuests : undefined} /> : (
        <div className="alert warn">{tr(lang, "Сканер працює для опублікованих подій, що ще не минули.", "The scanner works for published events that haven't ended.")}</div>
      ))}

      {tab === "staff" && owner && (
        <div className="card">
          <h3 style={{ marginBottom: 6 }}>{tr(lang, "Запросіть людину сканувати квитки", "Invite someone to scan tickets")}</h3>
          <p className="muted small" style={{ marginBottom: 16 }}>{tr(lang, "Помічник відкриє посилання, увійде — і зможе лише перевіряти QR на вході цієї події. Посилання одноразове й діє 3 дні.", "Your helper opens the link, signs in, and can only check QR codes at this event's door. Single-use, valid for 3 days.")}</p>
          {invite ? (
            <div className="stack">
              <input className="input" readOnly value={invite} onFocus={(e) => e.currentTarget.select()} aria-label={tr(lang, "Посилання-запрошення", "Invite link")} />
              <div className="row gap wrap">
                <button className="btn" onClick={() => { void navigator.clipboard.writeText(invite); showToast(tr(lang, "Скопійовано", "Copied")); }}><Copy className="ico sm" />{tr(lang, "Скопіювати", "Copy")}</button>
                <button className="btn ghost" onClick={makeInvite} disabled={busy}>{tr(lang, "Нове посилання", "New link")}</button>
              </div>
            </div>
          ) : (
            <button className="btn" onClick={makeInvite} disabled={busy}><UserPlus className="ico sm" />{tr(lang, "Створити посилання", "Create link")}</button>
          )}
          {staff && (staff.staff.length > 0 || staff.invites.some((i) => i.status === "pending")) && (
            <div style={{ marginTop: 22 }}>
              <div className="muted xs strong" style={{ textTransform: "uppercase", letterSpacing: ".06em", marginBottom: 6 }}>{tr(lang, "Мають доступ", "Has access")}</div>
              {staff.staff.map((m) => (
                <div key={m.user_id} className="guest">
                  <span className="avatar sm">{initials(m.name)}</span>
                  <span className="grow strong">{m.name}</span>
                  <span className="badge ok">{tr(lang, "сканує", "scanner")}</span>
                  <button className="btn quiet sm" disabled={busy} onClick={() => dropStaff(m.user_id)} aria-label={tr(lang, `Забрати доступ у ${m.name}`, `Remove ${m.name}`)}><X className="ico sm" /></button>
                </div>
              ))}
              {staff.invites.filter((i) => i.status === "pending").map((i) => (
                <div key={i.id} className="guest">
                  <span className="avatar sm" style={{ background: "var(--surface-3)" }}><UserPlus className="ico sm" /></span>
                  <span className="grow muted">{tr(lang, "Запрошення не прийняте", "Invite not accepted yet")} · {tr(lang, "до", "until")} {shortWhen(i.expires_at, lang)}</span>
                  <button className="btn quiet sm" disabled={busy} onClick={() => revoke(i.id)}>{tr(lang, "Відкликати", "Revoke")}</button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {owner && event.status === "published" && (
        <section className="section card">
          <h3 style={{ marginBottom: 12 }}>{tr(lang, "Керування", "Controls")}</h3>
          <div className="row gap wrap">
            <button className="btn secondary sm" onClick={toggleBooking} disabled={busy}>{event.sales_closed ? tr(lang, "Відкрити бронювання", "Open booking") : tr(lang, "Закрити бронювання", "Close booking")}</button>
            <button className="btn danger-ghost sm" onClick={() => setConfirm("cancel")} disabled={busy}>{tr(lang, "Скасувати подію", "Cancel event")}</button>
          </div>
        </section>
      )}

      {confirm === "cancel" && (
        <Modal title={tr(lang, "Скасувати подію?", "Cancel the event?")} onClose={() => !busy && setConfirm(null)}>
          <div className="stack lg">
            <p className="muted">{tr(lang, "Подія зникне з афіші, а гості побачать, що її скасовано. Цю дію не можна відмінити.", "The event leaves the listings and guests see it was cancelled. This can't be undone.")}</p>
            <div className="row gap">
              <button className="btn ghost grow" onClick={() => setConfirm(null)} disabled={busy}>{tr(lang, "Ні", "No")}</button>
              <button className="btn danger grow" onClick={doCancel} disabled={busy}>{tr(lang, "Так, скасувати", "Yes, cancel")}</button>
            </div>
          </div>
        </Modal>
      )}
      <Toast msg={toast} />
    </main>
  );
}

export default function ManageEventPage() {
  return <Suspense fallback={null}><ManageInner /></Suspense>;
}
