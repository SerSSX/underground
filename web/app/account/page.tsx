"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import {
  getToken, clearToken, getMe, patchMe, exportMyData, deleteMyAccount,
  getPayoutStatus, connectPayouts, errorMessage, type UserDto, type PayoutStatus,
} from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";
import Modal from "../components/Modal";

export default function AccountPage() {
  const router = useRouter();
  const { lang } = useLang();
  const [me, setMe] = useState<UserDto | null>(null);
  const [kyc, setKyc] = useState<PayoutStatus | null>(null);
  const [dob, setDob] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Payout onboarding: the only thing we ask for is the country. Everything else — identity,
  // bank account — is entered on the provider's own pages and never passes through us.
  const [country, setCountry] = useState("");
  const [connecting, setConnecting] = useState(false);
  // delete-account modal
  const [showDelete, setShowDelete] = useState(false);
  const [deletePw, setDeletePw] = useState("");

  useEffect(() => {
    if (!getToken()) {
      router.push("/login?next=/account");
      return;
    }
    getMe().then(setMe).catch((e) => setError(errorMessage(e)));
    getPayoutStatus().then(setKyc).catch(() => {});
  }, [router]);

  async function saveDob() {
    setError(null); setMsg(null);
    try {
      await patchMe({ dob });
      setMsg(tr(lang, "Дату народження збережено", "Date of birth saved"));
    } catch (e) { setError(errorMessage(e)); }
  }

  async function doExport() {
    setError(null);
    try {
      const data = await exportMyData();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url; a.download = "underground-my-data.json"; a.click();
      URL.revokeObjectURL(url);
    } catch (e) { setError(errorMessage(e)); }
  }

  async function doDelete() {
    if (!deletePw) return;
    setError(null);
    try {
      await deleteMyAccount(deletePw);
      clearToken();
      router.push("/");
    } catch (e) { setError(errorMessage(e)); }
  }

  async function doConnect() {
    setError(null); setMsg(null); setConnecting(true);
    try {
      // An account that already exists keeps its country; only a first connection asks for one.
      const { onboarding_url } = await connectPayouts((kyc?.country ?? country).trim());
      // Single-use and short-lived: go straight there rather than keeping it around.
      window.location.href = onboarding_url;
    } catch (e) { setError(errorMessage(e)); setConnecting(false); }
  }

  if (error && !me) return <main className="container narrow"><div className="error">{error}</div></main>;
  if (!me) return <main className="container narrow"><p className="muted">{tr(lang, "Завантаження…", "Loading…")}</p></main>;

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24 }}>{tr(lang, "Акаунт", "Account")}</h1>
      <p className="muted">{me.name} · {me.email}</p>
      {error && <div className="error">{error}</div>}
      {msg && <div className="muted" style={{ color: "var(--accent)" }}>{msg}</div>}

      <section style={{ marginTop: 24 }}>
        <h2 style={{ fontSize: 18 }}>{tr(lang, "Дата народження", "Date of birth")}</h2>
        <p className="muted">{tr(lang, "Потрібна для подій із віковим обмеженням.", "Required for age-restricted events.")}</p>
        <div className="row gap" style={{ alignItems: "center" }}>
          <input className="input" type="date" value={dob} onChange={(e) => setDob(e.target.value)} />
          <button className="btn sm" onClick={saveDob} disabled={!dob}>{tr(lang, "Зберегти", "Save")}</button>
        </div>
      </section>

      <section style={{ marginTop: 24 }}>
        <h2 style={{ fontSize: 18 }}>{tr(lang, "Виплати організатора", "Organizer payouts")}</h2>
        {!kyc ? null : !kyc.connected ? (
          <>
            <p className="muted">
              {tr(lang,
                "Щоб продавати платні квитки, підключіть виплати. Свої дані й банківський рахунок ви вводите на сторінці платіжної системи — ми їх не бачимо і не зберігаємо.",
                "To sell paid tickets, connect payouts. You enter your details and bank account on the payment provider's own page — we never see or store them.")}
            </p>
            <div className="form">
              <input
                className="input"
                placeholder={tr(lang, "Країна (2 літери, напр. PL)", "Country (2 letters, e.g. PL)")}
                value={country}
                maxLength={2}
                onChange={(e) => setCountry(e.target.value.toUpperCase())}
              />
              <button className="btn sm" onClick={doConnect} disabled={country.trim().length !== 2 || connecting}>
                {tr(lang, "Підключити виплати", "Connect payouts")}
              </button>
            </div>
          </>
        ) : (
          <>
            <p>
              {tr(lang, "Приймання оплат:", "Taking payments:")}{" "}
              <span className={kyc.charges_enabled ? "pill ok" : "pill warn"}>
                {kyc.charges_enabled ? tr(lang, "увімкнено", "enabled") : tr(lang, "перевіряється", "in review")}
              </span>
              {"  "}
              {tr(lang, "Виплати:", "Payouts:")}{" "}
              <span className={kyc.payouts_enabled ? "pill ok" : "pill warn"}>
                {kyc.payouts_enabled ? tr(lang, "увімкнено", "enabled") : tr(lang, "перевіряється", "in review")}
              </span>
            </p>
            {kyc.requirements_due.length > 0 && (
              <p className="muted">
                {tr(lang, "Платіжна система чекає документи:", "The payment provider is waiting for:")}{" "}
                {kyc.requirements_due.join(", ")}
              </p>
            )}
            {!kyc.charges_enabled && (
              <p className="muted">
                {tr(lang, "Поки перевірка триває, публікація платних подій недоступна.",
                         "While the review is in progress, publishing paid events is unavailable.")}
              </p>
            )}
            <button className="btn sm ghost" onClick={doConnect} disabled={connecting}>
              {tr(lang, "Змінити реквізити", "Update details")}
            </button>
          </>
        )}
      </section>

      <section style={{ marginTop: 24 }}>
        <h2 style={{ fontSize: 18 }}>{tr(lang, "Ваші дані (GDPR)", "Your data (GDPR)")}</h2>
        <div className="row gap" style={{ flexWrap: "wrap" }}>
          <button className="btn sm ghost" onClick={doExport}>{tr(lang, "Експортувати мої дані", "Export my data")}</button>
          <button className="btn sm danger" onClick={() => { setDeletePw(""); setShowDelete(true); }}>{tr(lang, "Видалити акаунт", "Delete account")}</button>
        </div>
        <p className="muted" style={{ fontSize: 13, marginTop: 8 }}>
          {tr(lang, "Видалення анонімізує вашу особу; фінансові записи зберігаються знеособленими для податкових вимог.", "Deletion anonymizes your identity; financial records are kept de-identified for tax requirements.")}
        </p>
      </section>

      <p className="muted" style={{ marginTop: 24, fontSize: 13 }}>
        <a href="/privacy">{tr(lang, "Конфіденційність", "Privacy")}</a> ·{" "}
        <a href="/terms">{tr(lang, "Умови", "Terms")}</a>
      </p>

      {showDelete && (
        <Modal title={tr(lang, "Видалити акаунт", "Delete account")} onClose={() => setShowDelete(false)}>
          <form className="form" onSubmit={(e) => { e.preventDefault(); doDelete(); }}>
            <p className="muted" style={{ fontSize: 13 }}>
              {tr(lang, "Дію не можна скасувати. Підтвердіть пароль.", "This cannot be undone. Confirm your password.")}
            </p>
            <input
              className="input"
              type="password"
              placeholder={tr(lang, "Пароль", "Password")}
              value={deletePw}
              onChange={(e) => setDeletePw(e.target.value)}
              required
              autoComplete="current-password"
            />
            <button className="btn block danger" disabled={!deletePw}>{tr(lang, "Видалити назавжди", "Delete permanently")}</button>
          </form>
        </Modal>
      )}
    </main>
  );
}
