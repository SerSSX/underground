"use client";

import { useCallback, useEffect, useState } from "react";
import Shell from "../components/Shell";
import DataTable from "../components/DataTable";
import StatusBadge from "../components/StatusBadge";
import { apiGet, apiSend, setKyc, errorMessage, type AdminUser } from "../../lib/api";

export default function UsersPage() {
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [q, setQ] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback((query: string) => {
    setLoading(true);
    setError(null);
    apiGet<AdminUser[]>(`/api/admin/users?q=${encodeURIComponent(query)}`)
      .then(setUsers)
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => load(""), [load]);

  async function patch(id: string, body: { blocked?: boolean; role?: string; verified?: boolean }) {
    setError(null);
    try {
      const updated = await apiSend<AdminUser>("PATCH", `/api/admin/users/${id}`, body);
      setUsers((list) => list.map((u) => (u.id === id ? updated : u)));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function kyc(id: string, status: string) {
    setError(null);
    try {
      await setKyc(id, status);
      setUsers((list) => list.map((u) => (u.id === id ? { ...u, kyc_status: status } : u)));
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <Shell title="Користувачі" subtitle="Пошук, ролі, блокування та KYC">
      {error && <div className="error" style={{ marginBottom: 12 }}>{error}</div>}
      <DataTable
        colSpan={6}
        loading={loading}
        empty={users.length === 0}
        emptyIcon="👤"
        emptyText="Користувачів не знайдено."
        toolbar={
          <>
            <input
              className="input"
              style={{ maxWidth: 320 }}
              placeholder="Пошук за email чи іменем…"
              value={q}
              onChange={(e) => setQ(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && load(q)}
            />
            <button className="btn sm" onClick={() => load(q)}>Шукати</button>
          </>
        }
        headers={
          <tr><th>Email</th><th>Ім&apos;я</th><th>Роль</th><th>Стан</th><th>KYC</th><th>Платіжка</th><th className="actions">Дії</th></tr>
        }
      >
        {users.map((u) => (
          <tr key={u.id}>
            <td>{u.email} {u.verified && <span className="badge2 s-ok" title="Верифікований організатор">✓</span>}</td>
            <td>{u.name}</td>
            <td><span className={`badge2 ${u.role === "ADMIN" ? "s-accent" : "s-neutral"}`}>{u.role}</span></td>
            <td><StatusBadge status={u.blocked ? "blocked" : "active"} /></td>
            <td><StatusBadge status={u.kyc_status} /></td>
            <td title="Стан акаунта у платіжній системі — лише читання, керує провайдер">
              {!u.psp_connected
                ? "—"
                : `${u.psp_charges_enabled ? "оплати ✓" : "оплати …"} / ${u.psp_payouts_enabled ? "виплати ✓" : "виплати …"}`}
            </td>
            <td className="actions">
              <div className="row" style={{ gap: 6, justifyContent: "flex-end" }}>
                <button className={u.blocked ? "btn sm ok" : "btn sm danger"} onClick={() => patch(u.id, { blocked: !u.blocked })}>
                  {u.blocked ? "Розблокувати" : "Заблокувати"}
                </button>
                <button className="btn sm ghost" onClick={() => patch(u.id, { verified: !u.verified })}>
                  {u.verified ? "Зняти ✓" : "Верифікувати"}
                </button>
                {u.kyc_status === "verified"
                  ? <button className="btn sm danger" onClick={() => kyc(u.id, "rejected")}>KYC ✗</button>
                  : <button className="btn sm ghost" onClick={() => kyc(u.id, "verified")}>KYC ✓</button>}
                <button className="btn sm ghost" onClick={() => patch(u.id, { role: u.role === "ADMIN" ? "USER" : "ADMIN" })}>
                  {u.role === "ADMIN" ? "→ USER" : "→ ADMIN"}
                </button>
              </div>
            </td>
          </tr>
        ))}
      </DataTable>
    </Shell>
  );
}
