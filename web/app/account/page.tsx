"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import {
  getToken, clearToken, getMe, patchMe, exportMyData, deleteMyAccount,
  getKyc, submitKyc, errorMessage, type UserDto, type KycView,
} from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";
import Modal from "../components/Modal";

export default function AccountPage() {
  const router = useRouter();
  const { lang } = useLang();
  const [me, setMe] = useState<UserDto | null>(null);
  const [kyc, setKyc] = useState<KycView | null>(null);
  const [dob, setDob] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // KYC form
  const [kName, setKName] = useState("");
  const [iban, setIban] = useState("");
  const [swift, setSwift] = useState("");
  // delete-account modal
  const [showDelete, setShowDelete] = useState(false);
  const [deletePw, setDeletePw] = useState("");

  useEffect(() => {
    if (!getToken()) {
      router.push("/login?next=/account");
      return;
    }
    getMe().then(setMe).catch((e) => setError(errorMessage(e)));
    getKyc().then(setKyc).catch(() => {});
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

  async function doKyc() {
    setError(null); setMsg(null);
    try {
      await submitKyc(kName.trim(), iban.trim(), swift.trim());
      setMsg(tr(lang, "Реквізити надіслано на перевірку", "Details submitted for review"));
      setKyc(await getKyc());
    } catch (e) { setError(errorMessage(e)); }
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
        <h2 style={{ fontSize: 18 }}>{tr(lang, "Виплати організатора (KYC)", "Organizer payouts (KYC)")}</h2>
        <p>
          {tr(lang, "Статус:", "Status:")}{" "}
          <span className={kyc?.kyc_status === "verified" ? "pill ok" : kyc?.kyc_status === "rejected" ? "pill bad" : kyc?.kyc_status === "pending" ? "pill warn" : "pill"}>
            {kyc?.kyc_status ?? "—"}
          </span>
          {kyc?.has_bank_details ? <span className="muted">{tr(lang, " · реквізити на файлі", " · bank details on file")}</span> : ""}
        </p>
        <p className="muted">{tr(lang, "Потрібно перед першою виплатою.", "Required before your first payout.")}</p>
        {(() => {
          const ibanClean = iban.replace(/\s/g, "").toUpperCase();
          const ibanOk = /^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$/.test(ibanClean);
          const swiftOk = /^[A-Z0-9]{8}([A-Z0-9]{3})?$/.test(swift.trim().toUpperCase());
          const nameOk = kName.trim().length > 0;
          const verified = kyc?.kyc_status === "verified";
          return (
            <div className="form">
              <input className="input" placeholder={tr(lang, "Ім'я отримувача", "Recipient name")} value={kName} onChange={(e) => setKName(e.target.value)} disabled={verified} />
              <input className="input" placeholder="IBAN (UA / DE…)" value={iban} onChange={(e) => setIban(e.target.value)} disabled={verified} aria-invalid={iban.length > 0 && !ibanOk} />
              {iban.length > 0 && !ibanOk && <div className="muted" style={{ fontSize: 12 }}>{tr(lang, "IBAN: 2 літери + 2 цифри + 11–30 символів", "IBAN: 2 letters + 2 digits + 11–30 chars")}</div>}
              <input className="input" placeholder="SWIFT / BIC" value={swift} onChange={(e) => setSwift(e.target.value)} disabled={verified} aria-invalid={swift.length > 0 && !swiftOk} />
              {swift.length > 0 && !swiftOk && <div className="muted" style={{ fontSize: 12 }}>{tr(lang, "SWIFT: 8 або 11 символів", "SWIFT: 8 or 11 characters")}</div>}
              {verified
                ? <p className="muted">{tr(lang, "Реквізити підтверджені. Зміна — через підтримку.", "Details verified. Change via support.")}</p>
                : <button className="btn sm" onClick={doKyc} disabled={!nameOk || !ibanOk || !swiftOk}>{tr(lang, "Надіслати на перевірку", "Submit for review")}</button>}
            </div>
          );
        })()}
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
