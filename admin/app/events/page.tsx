"use client";

import { useCallback, useEffect, useState } from "react";
import Shell from "../components/Shell";
import DataTable from "../components/DataTable";
import StatusBadge from "../components/StatusBadge";
import { apiGet, apiSend, errorMessage, type AdminEvent } from "../../lib/api";

const STATUSES = ["", "published", "hidden", "blocked", "cancelled"];

export default function EventsPage() {
  const [events, setEvents] = useState<AdminEvent[]>([]);
  const [status, setStatus] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback((s: string) => {
    setLoading(true);
    setError(null);
    apiGet<AdminEvent[]>(`/api/admin/events?status=${s}`)
      .then(setEvents)
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => load(status), [load, status]);

  async function moderate(id: string, newStatus: string) {
    setError(null);
    try {
      const updated = await apiSend<AdminEvent>("PATCH", `/api/admin/events/${id}`, { status: newStatus });
      setEvents((list) => list.map((e) => (e.id === id ? updated : e)));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <Shell title="Події" subtitle="Модерація: публікація, приховування, блокування">
      {error && <div className="error" style={{ marginBottom: 12 }}>{error}</div>}
      <DataTable
        colSpan={5}
        loading={loading}
        empty={events.length === 0}
        emptyIcon="🎟"
        emptyText="Подій не знайдено."
        toolbar={
          <select className="select" value={status} onChange={(e) => setStatus(e.target.value)}>
            {STATUSES.map((s) => (<option key={s} value={s}>{s === "" ? "усі статуси" : s}</option>))}
          </select>
        }
        headers={
          <tr><th>Подія</th><th>Організатор</th><th>Дата</th><th>Статус</th><th className="actions">Дії</th></tr>
        }
      >
        {events.map((e) => (
          <tr key={e.id}>
            <td>{e.title}<div className="muted" style={{ fontSize: 12 }}>{e.category}{e.city ? ` · ${e.city}` : ""}</div></td>
            <td className="mono">{e.organizer_email ?? "—"}</td>
            <td>{e.starts_at ? e.starts_at.slice(0, 16).replace("T", " ") : "—"}</td>
            <td>
              <StatusBadge status={e.status} />
              {e.sales_closed && <span className="badge2 s-warn" style={{ marginLeft: 4 }}>стоп-продажі</span>}
            </td>
            <td className="actions">
              <div className="row" style={{ gap: 6, justifyContent: "flex-end" }}>
                {e.status !== "published" && <button className="btn sm ok" onClick={() => moderate(e.id, "published")}>Опублікувати</button>}
                {e.status !== "hidden" && <button className="btn sm ghost" onClick={() => moderate(e.id, "hidden")}>Сховати</button>}
                {e.status !== "blocked" && <button className="btn sm danger" onClick={() => moderate(e.id, "blocked")}>Заблокувати</button>}
              </div>
            </td>
          </tr>
        ))}
      </DataTable>
    </Shell>
  );
}
