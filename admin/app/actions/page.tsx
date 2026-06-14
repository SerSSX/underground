"use client";

import { useEffect, useState } from "react";
import Shell from "../components/Shell";
import DataTable from "../components/DataTable";
import { listAdminActions, errorMessage, type AdminActionLog } from "../../lib/api";

// Human labels for the action codes the backend records.
const ACTION_LABEL: Record<string, string> = {
  user_update: "Користувач: зміна",
  event_moderate: "Подія: модерація",
  escrow_action: "Escrow: дія",
  post_delete: "Пост: видалення",
  review_delete: "Відгук: видалення",
  kyc_set: "KYC: рішення",
  payout_create: "Виплата",
  report_resolve: "Скарга: рішення",
  payment_settings_update: "Платіжки: зміна",
};

function fmt(ts: string): string {
  return new Date(ts).toLocaleString("uk-UA", { dateStyle: "short", timeStyle: "short" });
}

export default function ActionsPage() {
  const [actions, setActions] = useState<AdminActionLog[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("");

  useEffect(() => {
    listAdminActions()
      .then(setActions)
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, []);

  const shown = filter ? actions.filter((a) => a.action === filter) : actions;

  return (
    <Shell title="Журнал дій" subtitle="Незмінний аудит-трейл привілейованих дій модераторів — хто, що, коли і над чим">
      {error && <div className="error" style={{ marginBottom: 12 }}>{error}</div>}
      <DataTable
        colSpan={6}
        loading={loading}
        empty={shown.length === 0}
        emptyIcon="🗒"
        emptyText="Поки що немає записів."
        toolbar={
          <select className="select" value={filter} onChange={(e) => setFilter(e.target.value)}>
            <option value="">усі дії</option>
            {Object.entries(ACTION_LABEL).map(([k, v]) => (<option key={k} value={k}>{v}</option>))}
          </select>
        }
        headers={
          <tr><th>Час</th><th>Модератор</th><th>Дія</th><th>Тип</th><th>Ціль</th><th>Деталі</th></tr>
        }
      >
        {shown.map((a) => (
          <tr key={a.id}>
            <td className="muted" style={{ whiteSpace: "nowrap" }}>{fmt(a.created_at)}</td>
            <td>{a.admin_email ?? a.admin_name ?? <span className="muted">видалений</span>}</td>
            <td><span className="badge2 s-accent">{ACTION_LABEL[a.action] ?? a.action}</span></td>
            <td>{a.target_type}</td>
            <td className="mono" title={a.target_id ?? ""}>{a.target_id ? `${a.target_id.slice(0, 8)}…` : "—"}</td>
            <td className="mono">{JSON.stringify(a.detail)}</td>
          </tr>
        ))}
      </DataTable>
    </Shell>
  );
}
