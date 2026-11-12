"use client";

import Link from "next/link";
import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { acceptStaffInvite, getToken, errorMessage } from "../../../lib/api";
import { useLang, tr } from "../../../lib/i18n";
import { ScanLine } from "../../components/Icon";

/** Landing for the door-staff invite link the organiser shares: signs the person in, then grants scan rights. */
function AcceptInner() {
  const router = useRouter();
  const query = useSearchParams().get("token");
  // The invite link keeps its token after "#", so it never reaches a server log or a Referer header.
  const [token, setToken] = useState(query ?? "");
  useEffect(() => {
    const m = window.location.hash.match(/token=([^&]+)/);
    if (m) setToken(decodeURIComponent(m[1]));
  }, []);
  const { lang } = useLang();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!token) return;
    if (!getToken()) router.replace(`/login?next=${encodeURIComponent(`/staff/accept#token=${token}`)}`);
  }, [router, token]);

  async function accept() {
    setBusy(true);
    setError(null);
    try {
      const r = await acceptStaffInvite(token);
      router.push(r?.event_id ? `/organizer/${r.event_id}?tab=scan` : "/organizer");
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  }

  return (
    <main className="container tight">
      <div className="card" style={{ marginTop: 32, textAlign: "center", padding: 32 }}>
        <div className="empty" style={{ border: 0, padding: 0 }}>
          <div className="ei"><ScanLine className="ico lg" /></div>
        </div>
        <h1 style={{ fontSize: 26 }}>{tr(lang, "Вас запросили сканувати квитки", "You're invited to scan tickets")}</h1>
        <p className="muted" style={{ margin: "10px 0 22px" }}>{tr(lang, "Ви зможете лише перевіряти QR гостей на вході цієї події. Доступу до редагування чи списку гостей не буде.", "You'll only be able to check guests' QR codes at the door of this event — no editing, no guest list.")}</p>
        {error && <div className="alert error" style={{ marginBottom: 14 }}>{error}</div>}
        <button className="btn block lg" onClick={accept} disabled={busy || !token}>{busy ? "…" : tr(lang, "Прийняти запрошення", "Accept invite")}</button>
        <Link className="btn quiet block" href="/" style={{ marginTop: 8 }}>{tr(lang, "Не зараз", "Not now")}</Link>
      </div>
    </main>
  );
}

export default function StaffAcceptPage() {
  return <Suspense fallback={null}><AcceptInner /></Suspense>;
}
