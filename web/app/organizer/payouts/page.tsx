"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { organizerPayouts, organizerStats, getToken, errorMessage, type MyPayout, type OrganizerStats } from "../../../lib/api";
import { useLang, tr, fmtDateL } from "../../../lib/i18n";

const STATUS_CLS: Record<string, string> = { paid: "pill ok", pending: "pill warn", failed: "pill bad" };

export default function OrganizerPayoutsPage() {
  const router = useRouter();
  const { lang } = useLang();
  const [payouts, setPayouts] = useState<MyPayout[] | null>(null);
  const [stats, setStats] = useState<OrganizerStats | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!getToken()) {
      router.push("/login?next=/organizer/payouts");
      return;
    }
    organizerPayouts().then(setPayouts).catch((e) => setError(errorMessage(e)));
    organizerStats().then(setStats).catch(() => {});
  }, [router]);

  return (
    <main className="container">
      <div className="row gap" style={{ marginBottom: 4 }}>
        <Link className="link" href="/organizer">{tr(lang, "← Кабінет", "← Dashboard")}</Link>
      </div>
      <h1 style={{ fontSize: 24 }}>{tr(lang, "Виплати", "Payouts")}</h1>
      {error && <div className="error">{error}</div>}

      {stats && (
        <div className="stats" style={{ marginTop: 12 }}>
          <div className="stat"><div className="v">{stats.escrow_held} ₴</div><div className="l">{tr(lang, "Тримається в escrow", "Held in escrow")}</div></div>
          <div className="stat"><div className="v">{stats.escrow_cleared} ₴</div><div className="l">{tr(lang, "Готово до виплати", "Ready for payout")}</div></div>
          <div className="stat"><div className="v">{stats.paid_out} ₴</div><div className="l">{tr(lang, "Виплачено", "Paid out")}</div></div>
        </div>
      )}

      <p className="muted" style={{ marginTop: 12, fontSize: 13 }}>
        {tr(
          lang,
          "Кошти проходять escrow: тримаються під час продажу → очищуються після події → виплачуються платформою (за вирахуванням комісії). Тут — історія ваших виплат.",
          "Funds go through escrow: held during sale → cleared after the event → paid out by the platform (net of commission). Below is your settlement history.",
        )}
      </p>

      {payouts && payouts.length === 0 && (
        <p className="muted" style={{ marginTop: 16 }}>
          {tr(lang, "Виплат ще не було. Щойно кошти очистяться після події, платформа їх виплатить.", "No payouts yet. Once funds clear after your event, the platform settles them.")}
        </p>
      )}

      {payouts && payouts.length > 0 && (
        <table className="table" style={{ marginTop: 16 }}>
          <thead><tr><th>{tr(lang, "Дата", "Date")}</th><th>{tr(lang, "Сума", "Amount")}</th><th>{tr(lang, "Замовлень", "Orders")}</th><th>{tr(lang, "Статус", "Status")}</th></tr></thead>
          <tbody>
            {payouts.map((p) => (
              <tr key={p.id}>
                <td>{fmtDateL(p.created_at, lang)}</td>
                <td><strong>{p.amount} {p.currency}</strong></td>
                <td>{p.order_count}</td>
                <td>
                  <span className={STATUS_CLS[p.status] ?? "pill"}>{p.status}</span>
                  {p.failure_reason && <div className="muted" style={{ fontSize: 12 }}>{p.failure_reason}</div>}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </main>
  );
}
