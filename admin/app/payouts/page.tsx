"use client";

import { useCallback, useEffect, useState } from "react";
import Shell from "../components/Shell";
import DataTable from "../components/DataTable";
import StatusBadge from "../components/StatusBadge";
import PayoutActionDrawer from "../components/PayoutActionDrawer";
import { apiSend, listPayouts, updatePayout, errorMessage, type Payout } from "../../lib/api";

const FILTERS: { v: string; label: string }[] = [
  { v: "", label: "Усі" },
  { v: "paid", label: "Виплачені" },
  { v: "pending", label: "Очікують" },
  { v: "failed", label: "Невдалі" },
];

export default function PayoutsPage() {
  const [payouts, setPayouts] = useState<Payout[]>([]);
  const [organizerId, setOrganizerId] = useState("");
  const [filter, setFilter] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [active, setActive] = useState<Payout | null>(null);

  const load = useCallback((status: string) => {
    setLoading(true);
    listPayouts(status || undefined)
      .then(setPayouts)
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, []);
  useEffect(() => load(filter), [load, filter]);

  async function settle() {
    if (!organizerId.trim()) return;
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      const created = await apiSend<Payout[]>("POST", `/api/admin/organizers/${organizerId.trim()}/payout`, undefined);
      setNote(created.length ? `Створено виплат: ${created.length}` : "Немає очищених коштів для виплати");
      setOrganizerId("");
      load(filter);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function applyStatus(id: string, status: string, reason?: string) {
    setError(null);
    setBusy(true);
    try {
      await updatePayout(id, status, reason);
      setActive(null);
      load(filter);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Shell title="Виплати організаторам" subtitle="Ручний сетлмент cleared-коштів за вирахуванням комісії">
      {note && <div className="ok" style={{ marginBottom: 8 }}>{note}</div>}
      {error && <div className="error" style={{ marginBottom: 12 }}>{error}</div>}
      <DataTable
        colSpan={6}
        loading={loading}
        empty={payouts.length === 0}
        emptyIcon="💸"
        emptyText="Виплат не знайдено."
        toolbar={
          <>
            {FILTERS.map((f) => (
              <button key={f.v || "all"} className={filter === f.v ? "btn sm" : "btn sm ghost"} onClick={() => setFilter(f.v)}>{f.label}</button>
            ))}
            <span style={{ flex: 1 }} />
            <input
              className="input"
              style={{ maxWidth: 280 }}
              placeholder="ID організатора (UUID)"
              value={organizerId}
              onChange={(e) => setOrganizerId(e.target.value)}
            />
            <button className="btn sm" onClick={settle} disabled={busy || !organizerId.trim()}>Провести виплату</button>
          </>
        }
        headers={
          <tr><th>Дата</th><th>Організатор</th><th>Сума</th><th>Замовлень</th><th>Статус</th><th className="actions">Дії</th></tr>
        }
      >
        {payouts.map((p) => (
          <tr key={p.id}>
            <td className="muted">{new Date(p.created_at).toLocaleString("uk-UA")}</td>
            <td className="mono">{p.organizer_email ?? p.organizer_id}</td>
            <td>{p.amount} {p.currency}</td>
            <td>{p.order_count}</td>
            <td>
              <StatusBadge status={p.status} />
              {p.failure_reason && <div className="muted" style={{ fontSize: 12 }}>{p.failure_reason}</div>}
            </td>
            <td className="actions">
              <button className="btn sm" onClick={() => setActive(p)}>Керувати</button>
            </td>
          </tr>
        ))}
      </DataTable>

      <PayoutActionDrawer
        payout={active}
        busy={busy}
        onClose={() => setActive(null)}
        onConfirm={(status, reason) => active && applyStatus(active.id, status, reason)}
      />
    </Shell>
  );
}
