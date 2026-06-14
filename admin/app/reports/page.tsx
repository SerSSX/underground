"use client";

import { useCallback, useEffect, useState } from "react";
import Shell from "../components/Shell";
import DataTable from "../components/DataTable";
import StatusBadge from "../components/StatusBadge";
import ReportDrawer from "../components/ReportDrawer";
import {
  listReports, resolveReport, blockUser, moderateEvent, adminDeletePost, adminDeleteReview,
  errorMessage, type Report,
} from "../../lib/api";

const FILTERS: { v: string; label: string }[] = [
  { v: "open", label: "Відкриті" },
  { v: "reviewed", label: "Переглянуті" },
  { v: "resolved", label: "Закриті" },
  { v: "", label: "Усі" },
];

export default function ReportsPage() {
  const [reports, setReports] = useState<Report[]>([]);
  const [filter, setFilter] = useState("open");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState<Report | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback((status: string) => {
    setLoading(true);
    setError(null);
    listReports(status || undefined)
      .then(setReports)
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => load(filter), [load, filter]);

  async function resolve(id: string, status: string, note: string) {
    setBusy(true);
    setError(null);
    try {
      await resolveReport(id, status, note);
      setActive(null);
      load(filter);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  // Enforcement: act on the reported target, then auto-resolve the report (with the note).
  async function enforce(r: Report, note: string) {
    setBusy(true);
    setError(null);
    try {
      if (r.target_type === "event") await moderateEvent(r.target_id, "blocked");
      else if (r.target_type === "user") await blockUser(r.target_id);
      else if (r.target_type === "post") await adminDeletePost(r.target_id);
      else if (r.target_type === "review") await adminDeleteReview(r.target_id);
      await resolveReport(r.id, "resolved", note);
      setActive(null);
      load(filter);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Shell title="Скарги" subtitle="Черга модерації та контент-дії">
      {error && <div className="error" style={{ marginBottom: 12 }}>{error}</div>}
      <DataTable
        colSpan={6}
        loading={loading}
        empty={reports.length === 0}
        emptyIcon="⚑"
        emptyText="Немає скарг у цьому статусі."
        toolbar={FILTERS.map((f) => (
          <button key={f.v || "all"} className={filter === f.v ? "btn sm" : "btn sm ghost"} onClick={() => setFilter(f.v)}>
            {f.label}
          </button>
        ))}
        headers={
          <tr><th>Тип</th><th>Ціль</th><th>Причина</th><th>Скаржник</th><th>Статус</th><th className="actions">Дії</th></tr>
        }
      >
        {reports.map((r) => (
          <tr key={r.id}>
            <td><span className="badge2 s-neutral">{r.target_type}</span></td>
            <td className="mono" title={r.target_id}>{r.target_id.slice(0, 8)}…</td>
            <td>{r.reason}{r.details && <div className="muted" style={{ fontSize: 12 }}>{r.details}</div>}</td>
            <td className="mono">{r.reporter_email ?? "—"}</td>
            <td><StatusBadge status={r.status} /></td>
            <td className="actions">
              <button className="btn sm" onClick={() => setActive(r)}>Розглянути</button>
            </td>
          </tr>
        ))}
      </DataTable>

      <ReportDrawer
        report={active}
        all={reports}
        busy={busy}
        onClose={() => setActive(null)}
        onEnforce={enforce}
        onResolve={(status, note) => active && resolve(active.id, status, note)}
      />
    </Shell>
  );
}
