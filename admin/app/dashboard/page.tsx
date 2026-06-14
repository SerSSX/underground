"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import Shell from "../components/Shell";
import StatCard from "../components/StatCard";
import { apiGet, errorMessage, listReports, type AdminStats } from "../../lib/api";

export default function DashboardPage() {
  const [stats, setStats] = useState<AdminStats | null>(null);
  const [openReports, setOpenReports] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    apiGet<AdminStats>("/api/admin/stats").then(setStats).catch((e) => setError(errorMessage(e)));
    // Triage signal: how many reports await moderation.
    listReports("open").then((r) => setOpenReports(r.length)).catch(() => setOpenReports(null));
  }, []);

  return (
    <Shell title="Дашборд" subtitle="Стан платформи на цей момент">
      {error && <div className="error">{error}</div>}

      <div className="alertstrip">
        <Link className={openReports ? "alert hot" : "alert"} href="/reports">
          <span className="n">{openReports ?? "—"}</span> скарг у черзі
        </Link>
        <Link className="alert" href="/orders">⚑ Спори та escrow</Link>
        <Link className="alert" href="/payouts">💸 Виплати</Link>
      </div>

      {!stats && !error ? (
        <div className="statgrid">
          {Array.from({ length: 5 }).map((_, i) => (
            <div className="statcard" key={i}><span className="skel" style={{ height: 48 }} /></div>
          ))}
        </div>
      ) : stats ? (
        <div className="statgrid">
          <StatCard icon="₴" value={`${stats.gmv} ₴`} label="GMV (оплачені замовлення)" />
          <StatCard icon="%" value={`${stats.platform_fee} ₴`} label="Комісія платформи" />
          <StatCard icon="👥" value={stats.users_count} label="Користувачів" />
          <StatCard icon="🎟" value={stats.events_count} label="Подій" />
          <StatCard icon="✓" value={stats.tickets_count} label="Активних квитків" />
        </div>
      ) : null}
    </Shell>
  );
}
