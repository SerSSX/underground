"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { myEvents, organizerStats, getToken, errorMessage, type EventDto, type OrganizerStats } from "../../lib/api";
import { useLang, tr, fmtDateL } from "../../lib/i18n";

export default function OrganizerPage() {
  const router = useRouter();
  const { lang } = useLang();
  const [stats, setStats] = useState<OrganizerStats | null>(null);
  const [events, setEvents] = useState<EventDto[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!getToken()) {
      router.push("/login?next=/organizer");
      return;
    }
    organizerStats().then(setStats).catch((e) => setError(errorMessage(e)));
    myEvents().then(setEvents).catch((e) => setError(errorMessage(e)));
  }, [router]);

  return (
    <main className="container">
      <div className="row spread">
        <h1 style={{ fontSize: 24 }}>{tr(lang, "Кабінет організатора", "Organizer dashboard")}</h1>
        <Link className="btn sm" href="/organizer/new">{tr(lang, "+ Створити подію", "+ Create event")}</Link>
      </div>
      <div className="row gap" style={{ marginTop: 6 }}>
        <Link className="link" href="/organizer/analytics">{tr(lang, "📈 Аналітика", "📈 Analytics")}</Link>
        <Link className="link" href="/organizer/payouts">{tr(lang, "💸 Виплати", "💸 Payouts")}</Link>
      </div>
      {error && <div className="error">{error}</div>}
      {stats && (
        <div className="stats">
          <div className="stat"><div className="v">{stats.revenue} ₴</div><div className="l">{tr(lang, "Виручка", "Revenue")}</div></div>
          <div className="stat"><div className="v">{stats.escrow_held} ₴</div><div className="l">{tr(lang, "В ескроу", "In escrow")}</div></div>
          <div className="stat"><div className="v">{stats.escrow_cleared} ₴</div><div className="l">{tr(lang, "До виплати", "Payout")}</div></div>
          <div className="stat"><div className="v">{stats.paid_out} ₴</div><div className="l">{tr(lang, "Виплачено", "Paid out")}</div></div>
          <div className="stat"><div className="v">{stats.tickets_sold}</div><div className="l">{tr(lang, "Продано", "Sold")}</div></div>
          <div className="stat"><div className="v">{stats.tickets_validated}</div><div className="l">{tr(lang, "Відвідали", "Attended")}</div></div>
          <div className="stat"><div className="v">{stats.events_count}</div><div className="l">{tr(lang, "Подій", "Events")}</div></div>
        </div>
      )}
      <h2 style={{ fontSize: 17, marginTop: 18 }}>{tr(lang, "Мої події", "My events")}</h2>
      <table className="table">
        <thead><tr><th>{tr(lang, "Подія", "Event")}</th><th>{tr(lang, "Дата", "Date")}</th><th>{tr(lang, "Лишилось", "Left")}</th><th>{tr(lang, "Статус", "Status")}</th><th>{tr(lang, "Дії", "Actions")}</th></tr></thead>
        <tbody>
          {events.map((e) => (
            <tr key={e.id}>
              <td>{e.title}</td>
              <td>{fmtDateL(e.starts_at, lang) || "—"}</td>
              <td>{e.tickets_left}</td>
              <td>
                <span className={e.status === "published" ? "pill ok" : "pill warn"}>{e.status}</span>
                {e.sales_closed && <span className="pill warn" style={{ marginLeft: 4 }}>{tr(lang, "стоп", "stopped")}</span>}
              </td>
              <td className="row gap">
                <Link className="link" href={`/organizer/${e.id}/edit`}>{tr(lang, "Редагувати", "Edit")}</Link>
                <Link className="link" href={`/organizer/${e.id}`}>{tr(lang, "Відвідувачі", "Attendees")}</Link>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {events.length === 0 && <p className="muted">{tr(lang, "Ще немає подій — натисніть «Створити подію».", "No events yet — click “Create event”.")}</p>}
    </main>
  );
}
