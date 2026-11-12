"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { myEvents, organizerStats, getToken, errorMessage, isPast, type EventDto, type OrganizerStats } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";
import { shortWhen, plural } from "../../lib/ui";
import Poster from "../components/Poster";
import { Toast, useToast } from "../components/Toast";
import { Plus, ScanLine, Users, Pencil, Share2, CalendarPlus } from "../components/Icon";

type Tab = "upcoming" | "past" | "drafts";

export default function OrganizerPage() {
  const router = useRouter();
  const { lang } = useLang();
  const [stats, setStats] = useState<OrganizerStats | null>(null);
  const [events, setEvents] = useState<EventDto[] | null>(null);
  const [tab, setTab] = useState<Tab>("upcoming");
  const [error, setError] = useState<string | null>(null);
  const [toast, showToast] = useToast();

  useEffect(() => {
    if (!getToken()) {
      router.replace("/login?next=/organizer");
      return;
    }
    organizerStats().then(setStats).catch(() => {});
    myEvents().then(setEvents).catch((e) => { setError(errorMessage(e)); setEvents([]); });
  }, [router]);

  const groups = useMemo(() => {
    const all = events ?? [];
    return {
      upcoming: all.filter((e) => e.status === "published" && !isPast(e.starts_at)),
      past: all.filter((e) => e.status !== "draft" && (isPast(e.starts_at) || e.status !== "published")).reverse(),
      drafts: all.filter((e) => e.status === "draft"),
    };
  }, [events]);

  function share(e: EventDto) {
    const url = `${window.location.origin}/events/${e.id}`;
    if (navigator.share) void navigator.share({ title: e.title, url }).catch(() => {});
    else void navigator.clipboard.writeText(url).then(() => showToast(tr(lang, "Посилання скопійовано", "Link copied")));
  }

  const statusBadge = (e: EventDto) => {
    if (e.status === "draft") return <span className="badge warn">{tr(lang, "Чернетка", "Draft")}</span>;
    if (e.status === "cancelled") return <span className="badge danger">{tr(lang, "Скасовано", "Cancelled")}</span>;
    if (e.status === "hidden") return <span className="badge danger">{tr(lang, "Приховано модератором", "Hidden by moderator")}</span>;
    if (isPast(e.starts_at)) return <span className="badge">{tr(lang, "Минула", "Ended")}</span>;
    if (e.sales_closed) return <span className="badge warn">{tr(lang, "Бронювання закрите", "Booking closed")}</span>;
    return <span className="badge ok">{tr(lang, "Опубліковано", "Live")}</span>;
  };

  const list = groups[tab];

  return (
    <main className="container">
      <div className="page-head">
        <div>
          <h1>{tr(lang, "Мої події", "My events")}</h1>
          <p className="muted">{tr(lang, "Створюйте події, дивіться, хто прийде, і скануйте квитки на вході.", "Create events, see who's coming and scan tickets at the door.")}</p>
        </div>
        <Link className="btn" href="/organizer/new"><Plus className="ico sm" />{tr(lang, "Створити подію", "Create event")}</Link>
      </div>

      {stats && (
        <div className="stats">
          <div className="stat"><div className="v">{stats.events_count}</div><div className="l">{tr(lang, "Подій", "Events")}</div></div>
          <div className="stat"><div className="v">{stats.tickets_sold}</div><div className="l">{tr(lang, "Заброньовано місць", "Places booked")}</div></div>
          <div className="stat"><div className="v">{stats.tickets_validated}</div><div className="l">{tr(lang, "Прийшли", "Checked in")}</div></div>
        </div>
      )}

      <div className="tabs" role="tablist">
        {([["upcoming", "Майбутні", "Upcoming"], ["past", "Минулі", "Past"], ["drafts", "Чернетки", "Drafts"]] as [Tab, string, string][]).map(([k, uk, en]) => (
          <button key={k} role="tab" aria-selected={tab === k} className={tab === k ? "active" : ""} onClick={() => setTab(k)}>{tr(lang, uk, en)} <span className="count">{groups[k].length}</span></button>
        ))}
      </div>

      {error && <div className="alert error" style={{ marginBottom: 12 }}>{error}</div>}

      {events === null ? (
        <div className="stack">{[0, 1, 2].map((i) => <div key={i} className="skeleton" style={{ height: 116 }} />)}</div>
      ) : list.length === 0 ? (
        <div className="empty">
          <div className="ei"><CalendarPlus className="ico lg" /></div>
          <h3>{tab === "drafts" ? tr(lang, "Чернеток немає", "No drafts") : tab === "past" ? tr(lang, "Минулих подій ще немає", "No past events yet") : tr(lang, "Створіть першу подію", "Create your first event")}</h3>
          <p>{tr(lang, "Це займе пару хвилин: назва, дата, місце і кількість місць. Подія одразу зʼявиться в афіші.", "It takes a couple of minutes: title, date, place and capacity. It goes live at once.")}</p>
          <Link className="btn" href="/organizer/new"><Plus className="ico sm" />{tr(lang, "Створити подію", "Create event")}</Link>
        </div>
      ) : (
        <div className="stack">
          {list.map((e) => {
            const total = e.tickets_total ?? 0;
            const booked = total > 0 ? Math.max(0, total - e.tickets_left) : 0;
            const pct = total > 0 ? Math.round((booked / total) * 100) : 0;
            return (
              <article key={e.id} className="orow">
                <Link href={`/organizer/${e.id}`} className="thumb" aria-label={e.title}><Poster src={e.poster_url} title={e.title} category={e.category} /></Link>
                <div style={{ minWidth: 0 }}>
                  <div className="row gap wrap" style={{ marginBottom: 4 }}>{statusBadge(e)}</div>
                  <Link href={`/organizer/${e.id}`}><h3 style={{ whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{e.title}</h3></Link>
                  <div className="muted small">{shortWhen(e.starts_at, lang, e.timezone) || tr(lang, "Дата не вказана", "No date")}{e.city ? ` · ${e.city}` : ""}</div>
                  {total > 0 && (
                    <div style={{ marginTop: 10, maxWidth: 360 }}>
                      <div className={`progress ${pct >= 90 ? "warn" : ""}`}><i style={{ width: `${pct}%` }} /></div>
                      <div className="subtle xs" style={{ marginTop: 4 }}>
                        {tr(lang, `Заброньовано ${booked} з ${total} ${plural(total, ["місця", "місць", "місць"])}`, `${booked} of ${total} places booked`)}
                      </div>
                    </div>
                  )}
                </div>
                <div className="actions row gap wrap" style={{ justifyContent: "flex-end" }}>
                  {e.status === "published" && !isPast(e.starts_at) && <Link className="btn sm" href={`/organizer/${e.id}?tab=scan`}><ScanLine className="ico sm" />{tr(lang, "Сканер", "Scan")}</Link>}
                  <Link className="btn secondary sm" href={`/organizer/${e.id}`}><Users className="ico sm" />{tr(lang, "Гості", "Guests")}</Link>
                  <Link className="btn ghost sm icon" href={`/organizer/${e.id}/edit`} aria-label={tr(lang, "Редагувати", "Edit")} title={tr(lang, "Редагувати", "Edit")}><Pencil className="ico sm" /></Link>
                  {e.status === "published" && <button className="btn ghost sm icon" onClick={() => share(e)} aria-label={tr(lang, "Поділитися", "Share")} title={tr(lang, "Поділитися", "Share")}><Share2 className="ico sm" /></button>}
                </div>
              </article>
            );
          })}
        </div>
      )}
      <Toast msg={toast} />
    </main>
  );
}
