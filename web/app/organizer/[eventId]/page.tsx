"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import {
  eventTickets,
  listPromos,
  createPromo,
  deletePromo,
  getToken,
  errorMessage,
  type EventTicket,
  type PromoCode,
} from "../../../lib/api";

export default function AttendeesPage() {
  const { eventId } = useParams<{ eventId: string }>();
  const router = useRouter();
  const [tickets, setTickets] = useState<EventTicket[]>([]);
  const [promos, setPromos] = useState<PromoCode[]>([]);
  const [code, setCode] = useState("");
  const [pct, setPct] = useState("10");
  const [error, setError] = useState<string | null>(null);

  function loadPromos() {
    listPromos(eventId).then(setPromos).catch(() => {});
  }
  useEffect(() => {
    if (!getToken()) {
      router.push("/login");
      return;
    }
    eventTickets(eventId).then(setTickets).catch((e) => setError(errorMessage(e)));
    loadPromos();
  }, [eventId, router]);

  async function addPromo() {
    setError(null);
    try {
      await createPromo(eventId, code.trim(), Number(pct));
      setCode("");
      loadPromos();
    } catch (e) {
      setError(errorMessage(e));
    }
  }
  async function removePromo(id: string) {
    await deletePromo(id).catch((e) => setError(errorMessage(e)));
    loadPromos();
  }

  function exportCsv() {
    const rows = tickets.map((t) =>
      [t.owner_name, t.owner_email, t.status, t.price, t.created_at, t.validated_at ?? ""].join(","));
    const blob = new Blob([["name,email,status,price,bought_at,validated_at", ...rows].join("\n")], { type: "text/csv" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = "attendees.csv";
    a.click();
  }

  return (
    <main className="container">
      <div className="row spread">
        <h1 style={{ fontSize: 22 }}>Відвідувачі</h1>
        <Link className="link" href="/organizer">← до кабінету</Link>
      </div>
      {error && <div className="error">{error}</div>}
      <div className="row spread" style={{ margin: "10px 0" }}>
        <span className="muted">Квитків: {tickets.length}</span>
        <button className="btn sm" onClick={exportCsv} disabled={tickets.length === 0}>Експорт CSV</button>
      </div>
      <table className="table">
        <thead><tr><th>Ім&apos;я</th><th>Email</th><th>Статус</th><th>Сканований</th></tr></thead>
        <tbody>
          {tickets.map((t) => (
            <tr key={t.id}>
              <td>{t.owner_name}</td>
              <td>{t.owner_email}</td>
              <td><span className={t.status === "active" ? "pill ok" : t.status === "used" ? "pill" : "pill bad"}>{t.status}</span></td>
              <td>{t.validated_at ? t.validated_at.slice(0, 16).replace("T", " ") : "—"}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <h2 style={{ fontSize: 18, marginTop: 22 }}>Промокоди</h2>
      <p className="muted" style={{ fontSize: 13 }}>Покупці застосовують код на оплаті: <code>?promo=КОД</code></p>
      <div className="row gap" style={{ margin: "8px 0", flexWrap: "wrap" }}>
        <input className="input" style={{ maxWidth: 180 }} placeholder="КОД" value={code} onChange={(e) => setCode(e.target.value)} />
        <input className="input" style={{ maxWidth: 110 }} inputMode="numeric" placeholder="% знижки" value={pct} onChange={(e) => setPct(e.target.value)} />
        <button className="btn sm" onClick={addPromo} disabled={!code.trim()}>Створити код</button>
      </div>
      {promos.length > 0 && (
        <table className="table">
          <thead><tr><th>Код</th><th>Знижка</th><th>Використано</th><th></th></tr></thead>
          <tbody>
            {promos.map((p) => (
              <tr key={p.id}>
                <td><code>{p.code}</code></td>
                <td>−{p.percent_off}%</td>
                <td>{p.used_count}{p.max_uses != null ? ` / ${p.max_uses}` : ""}</td>
                <td><button className="btn sm danger" onClick={() => removePromo(p.id)}>Видалити</button></td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </main>
  );
}
