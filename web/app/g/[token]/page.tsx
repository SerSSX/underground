"use client";

// Gift claim landing (TASK-114 / WF-GiftClaim): browse-before-auth. A recipient opens the
// shared link, sees the gift with NO account, then claims it — signing in with a passwordless
// email code (an account is created on first use) in a couple of steps.

import { useEffect, useState } from "react";
import Link from "next/link";
import { useParams } from "next/navigation";
import {
  giftPreview,
  requestEmailCode,
  verifyEmailCode,
  claimTicket,
  getToken,
  setToken,
  errorMessage,
  fmtDate,
  type GiftPreviewDto,
} from "../../../lib/api";
import { useLang, tr } from "../../../lib/i18n";

export default function ClaimGiftPage() {
  const { lang } = useLang();
  const token = String(useParams().token || "");
  const [preview, setPreview] = useState<GiftPreviewDto | null>(null);
  const [loading, setLoading] = useState(true);
  const [gone, setGone] = useState(false);
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [codeSent, setCodeSent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);

  useEffect(() => {
    giftPreview(token)
      .then(setPreview)
      .catch(() => setGone(true))
      .finally(() => setLoading(false));
  }, [token]);

  async function run(fn: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  const claim = () => run(async () => { await claimTicket(token); setDone(true); });
  const sendCode = () => run(async () => { await requestEmailCode(email.trim()); setCodeSent(true); });
  const verifyAndClaim = () =>
    run(async () => {
      setToken(await verifyEmailCode(email.trim(), code.trim()));
      await claimTicket(token);
      setDone(true);
    });

  if (loading) {
    return (
      <main className="container narrow">
        <p className="muted">…</p>
      </main>
    );
  }
  if (gone || !preview) {
    return (
      <main className="container narrow" style={{ textAlign: "center" }}>
        <h1 style={{ fontSize: 22 }}>{tr(lang, "Подарунок недоступний", "Gift unavailable")}</h1>
        <p className="muted">
          {tr(lang, "Це посилання вже використали або воно прострочене.", "This link was already used or has expired.")}
        </p>
        <Link className="btn" href="/">{tr(lang, "До подій", "Browse events")}</Link>
      </main>
    );
  }
  if (done) {
    return (
      <main className="container narrow" style={{ textAlign: "center" }}>
        <h1 style={{ fontSize: 22 }}>{tr(lang, "Квиток твій!", "The ticket is yours!")}</h1>
        <p className="muted">{tr(lang, "Ми зберегли його в «Мої квитки».", "We saved it to your tickets.")}</p>
        <Link className="btn block" href="/me/tickets">{tr(lang, "Мої квитки", "My tickets")}</Link>
      </main>
    );
  }

  const card = {
    textAlign: "left" as const,
    marginTop: 12,
    padding: 14,
    border: "1px solid var(--border)",
    borderRadius: 12,
    background: "var(--surface)",
  };
  const loggedIn = !!getToken();
  return (
    <main className="container narrow" style={{ textAlign: "center" }}>
      <div style={{ fontSize: 40, lineHeight: 1 }} aria-hidden>🎁</div>
      <h1 style={{ fontSize: 22, marginTop: 8 }}>
        {tr(lang, `${preview.from_name} дарує тобі квиток`, `${preview.from_name} sent you a ticket`)}
      </h1>
      <div style={card}>
        <div style={{ fontWeight: 500 }}>{preview.event_title}</div>
        <div className="muted">
          {fmtDate(preview.event_starts_at)}
          {preview.event_city ? ` · ${preview.event_city}` : ""}
        </div>
      </div>
      {error && <div className="error" style={{ marginTop: 12 }}>{error}</div>}
      {loggedIn ? (
        <button className="btn block" style={{ marginTop: 12 }} disabled={busy} onClick={claim}>
          {busy ? "…" : tr(lang, "Забрати квиток", "Claim ticket")}
        </button>
      ) : !codeSent ? (
        <form className="form" style={{ marginTop: 12 }} onSubmit={(e) => { e.preventDefault(); void sendCode(); }}>
          <input
            className="input"
            type="email"
            placeholder="Email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            autoComplete="email"
          />
          <button className="btn block" disabled={busy || !email.trim()}>
            {busy ? "…" : tr(lang, "Отримати код на email", "Email me a code")}
          </button>
          <p className="muted" style={{ fontSize: 13 }}>
            {tr(lang, "Акаунт створиться автоматично.", "An account is created automatically.")}
          </p>
        </form>
      ) : (
        <form className="form" style={{ marginTop: 12 }} onSubmit={(e) => { e.preventDefault(); void verifyAndClaim(); }}>
          <input
            className="input"
            inputMode="numeric"
            placeholder={tr(lang, "Код з email", "Code from email")}
            value={code}
            onChange={(e) => setCode(e.target.value)}
            required
          />
          <button className="btn block" disabled={busy || !code.trim()}>
            {busy ? "…" : tr(lang, "Забрати квиток", "Claim ticket")}
          </button>
        </form>
      )}
      <p className="muted" style={{ fontSize: 12, marginTop: 14 }}>
        {tr(lang, "Безпечна передача · escrow", "Secure transfer · escrow")}
      </p>
    </main>
  );
}
