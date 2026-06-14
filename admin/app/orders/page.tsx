"use client";

import { useCallback, useEffect, useState } from "react";
import Shell from "../components/Shell";
import DataTable from "../components/DataTable";
import StatusBadge from "../components/StatusBadge";
import EscrowDrawer from "../components/EscrowDrawer";
import { apiGet, apiSend, errorMessage, type AdminOrder } from "../../lib/api";

export default function OrdersPage() {
  const [orders, setOrders] = useState<AdminOrder[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState<AdminOrder | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    setLoading(true);
    setError(null);
    apiGet<AdminOrder[]>("/api/admin/orders")
      .then(setOrders)
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => load(), [load]);

  async function escrow(id: string, action: string, force: boolean) {
    setBusy(true);
    setError(null);
    try {
      const updated = await apiSend<AdminOrder>("POST", `/api/admin/orders/${id}/escrow`, { action, force });
      setOrders((list) => list.map((o) => (o.id === id ? updated : o)));
      setActive(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Shell title="Замовлення та escrow" subtitle="Контроль коштів: повернення, заморозка, звільнення">
      {error && <div className="error" style={{ marginBottom: 12 }}>{error}</div>}
      <DataTable
        colSpan={6}
        loading={loading}
        empty={orders.length === 0}
        emptyIcon="🧾"
        emptyText="Замовлень ще немає."
        headers={
          <tr><th>Подія</th><th>Покупець</th><th>Сума</th><th>Статус</th><th>Escrow</th><th className="actions">Дії</th></tr>
        }
      >
        {orders.map((o) => {
          const actionable = o.escrow_status && o.escrow_status !== "paid_out" && o.escrow_status !== "refunded";
          return (
            <tr key={o.id}>
              <td>{o.event_title ?? "—"}</td>
              <td className="mono">{o.buyer_email ?? "—"}</td>
              <td>{o.amount} {o.currency}</td>
              <td><StatusBadge status={o.status} /></td>
              <td><StatusBadge status={o.escrow_status} /></td>
              <td className="actions">
                {actionable
                  ? <button className="btn sm" onClick={() => setActive(o)}>Керувати</button>
                  : <span className="muted">—</span>}
              </td>
            </tr>
          );
        })}
      </DataTable>

      <EscrowDrawer
        order={active}
        busy={busy}
        onClose={() => setActive(null)}
        onAction={(action, force) => active && escrow(active.id, action, force)}
      />
    </Shell>
  );
}
