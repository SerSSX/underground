"use client";

import { useState, type FormEvent } from "react";
import Link from "next/link";
import { forgotPassword, errorMessage } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";

export default function ForgotPasswordPage() {
  const { lang } = useLang();
  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await forgotPassword(email.trim());
      setSent(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24 }}>{tr(lang, "Відновлення паролю", "Reset password")}</h1>
      {sent ? (
        // Deliberately generic — we never reveal whether the email is registered.
        <p className="muted">
          {tr(
            lang,
            "Якщо для цієї адреси є акаунт, ми надіслали лист із посиланням для зміни паролю. Перевірте пошту.",
            "If an account exists for that address, we've sent a reset link. Check your inbox.",
          )}
        </p>
      ) : (
        <>
          <p className="muted">
            {tr(lang, "Введіть email — надішлемо посилання для зміни паролю.", "Enter your email and we'll send a reset link.")}
          </p>
          <form className="form" onSubmit={submit}>
            <input className="input" type="email" placeholder="Email" value={email} onChange={(e) => setEmail(e.target.value)} required autoComplete="email" />
            {error && <div className="error">{error}</div>}
            <button className="btn block" disabled={busy || !email.trim()}>{busy ? "…" : tr(lang, "Надіслати посилання", "Send reset link")}</button>
          </form>
        </>
      )}
      <p className="muted" style={{ marginTop: 14 }}>
        <Link className="link" href="/login">{tr(lang, "← Назад до входу", "← Back to sign in")}</Link>
      </p>
    </main>
  );
}
