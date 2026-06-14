"use client";

import { useEffect, useState } from "react";
import Shell from "../components/Shell";
import DataTable from "../components/DataTable";
import StatCard from "../components/StatCard";
import { apiGet, errorMessage } from "../../lib/api";

interface TelemetryStat {
  name: string;
  count: number;
  last_at: string | null;
}

// Human labels for the north-star funnel events (TASK-017 M4).
const LABELS: Record<string, string> = {
  signup: "Реєстрації",
  onboarding_done: "Завершили онбординг",
  discover_impression: "Покази «Для тебе»",
  save: "Збереження подій",
  ticket: "Отримані квитки",
  follow: "Підписки",
  share: "Поширення",
  follower_notify_open: "Відкриті сповіщення",
};
const ICONS: Record<string, string> = {
  signup: "👤", onboarding_done: "✓", discover_impression: "👁", save: "★",
  ticket: "🎟", follow: "➕", share: "↗", follower_notify_open: "🔔",
};

export default function TelemetryPage() {
  const [stats, setStats] = useState<TelemetryStat[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    apiGet<TelemetryStat[]>("/api/admin/telemetry").then(setStats).catch((e) => setError(errorMessage(e)));
  }, []);

  return (
    <Shell title="Телеметрія" subtitle="Продуктова воронка за останні 7 днів">
      {error && <div className="error" style={{ marginBottom: 12 }}>{error}</div>}
      {stats && stats.length > 0 && (
        <div className="statgrid">
          {stats.map((s) => (
            <StatCard key={s.name} icon={ICONS[s.name] ?? "•"} value={s.count} label={LABELS[s.name] ?? s.name} />
          ))}
        </div>
      )}
      <DataTable
        colSpan={3}
        loading={!stats && !error}
        empty={!!stats && stats.length === 0}
        emptyIcon="📈"
        emptyText="Подій за останні 7 днів ще немає."
        headers={<tr><th>Подія</th><th>Кількість</th><th>Остання</th></tr>}
      >
        {(stats ?? []).map((s) => (
          <tr key={s.name}>
            <td>{LABELS[s.name] ?? s.name} <span className="muted mono">({s.name})</span></td>
            <td>{s.count}</td>
            <td className="muted">{s.last_at ? new Date(s.last_at).toLocaleString("uk-UA") : "—"}</td>
          </tr>
        ))}
      </DataTable>
    </Shell>
  );
}
