"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { organizerAnalytics, getToken, errorMessage, type DailySales } from "../../../lib/api";
import { useLang, tr, fmtDateL } from "../../../lib/i18n";

export default function OrganizerAnalyticsPage() {
  const router = useRouter();
  const { lang } = useLang();
  const [rows, setRows] = useState<DailySales[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!getToken()) {
      router.push("/login?next=/organizer/analytics");
      return;
    }
    organizerAnalytics().then(setRows).catch((e) => setError(errorMessage(e)));
  }, [router]);

  const maxRev = rows && rows.length ? Math.max(...rows.map((r) => Number(r.revenue))) : 0;
  const totalRev = rows ? rows.reduce((s, r) => s + Number(r.revenue), 0) : 0;
  const totalTickets = rows ? rows.reduce((s, r) => s + r.tickets, 0) : 0;

  return (
    <main className="container">
      <div className="row gap" style={{ marginBottom: 4 }}>
        <Link className="link" href="/organizer">{tr(lang, "← Кабінет", "← Dashboard")}</Link>
      </div>
      <h1 style={{ fontSize: 24 }}>{tr(lang, "Аналітика продажів", "Sales analytics")}</h1>
      <p className="muted">{tr(lang, "Оплачені продажі за останні 30 днів.", "Paid sales over the last 30 days.")}</p>
      {error && <div className="error">{error}</div>}

      {rows && (
        <div className="stats" style={{ marginTop: 12 }}>
          <div className="stat"><div className="v">{totalRev.toFixed(2)} ₴</div><div className="l">{tr(lang, "Виручка (30 дн)", "Revenue (30d)")}</div></div>
          <div className="stat"><div className="v">{totalTickets}</div><div className="l">{tr(lang, "Квитків (30 дн)", "Tickets (30d)")}</div></div>
        </div>
      )}

      {rows && rows.length === 0 && <p className="muted" style={{ marginTop: 16 }}>{tr(lang, "Продажів за останні 30 днів ще немає.", "No paid sales in the last 30 days yet.")}</p>}

      {rows && rows.length > 0 && (
        <table className="table" style={{ marginTop: 16 }}>
          <thead><tr><th>{tr(lang, "День", "Day")}</th><th>{tr(lang, "Квитки", "Tickets")}</th><th>{tr(lang, "Виручка", "Revenue")}</th><th style={{ width: "45%" }}></th></tr></thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.day}>
                <td>{fmtDateL(r.day, lang)}</td>
                <td>{r.tickets}</td>
                <td>{r.revenue} ₴</td>
                <td>
                  <div style={{ background: "var(--accent)", height: 10, borderRadius: 6, width: `${maxRev ? (Number(r.revenue) / maxRev) * 100 : 0}%`, minWidth: 4 }} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </main>
  );
}
