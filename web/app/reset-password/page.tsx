"use client";

import { Suspense, useEffect, useState, type FormEvent } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { resetPassword, errorMessage } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";

function ResetForm() {
  const router = useRouter();
  const { lang } = useLang();
  // The link's token is kept in state and taken out of the address bar at once (TASK-304), so it does not stay
  // in the browser history or travel on in a Referer.
  const fromUrl = useSearchParams().get("token") || "";
  const [token, setTokenValue] = useState(fromUrl);
  useEffect(() => {
    if (!fromUrl) return;
    setTokenValue(fromUrl);
    window.history.replaceState(window.history.state, "", window.location.pathname);
  }, [fromUrl]);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);

  const mismatch = confirm.length > 0 && password !== confirm;

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (password !== confirm) {
      setError(tr(lang, "Паролі не співпадають", "Passwords don't match"));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await resetPassword(token, password);
      setDone(true);
      setTimeout(() => router.push("/login"), 1500);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  if (!token) {
    return (
      <main className="container narrow">
        <h1 style={{ fontSize: 24 }}>{tr(lang, "Зміна паролю", "Set a new password")}</h1>
        <div className="error">{tr(lang, "Посилання недійсне. Запросіть нове.", "Invalid link. Request a new one.")}</div>
        <p className="muted" style={{ marginTop: 14 }}>
          <Link className="link" href="/forgot-password">{tr(lang, "Відновити пароль", "Reset password")}</Link>
        </p>
      </main>
    );
  }

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24 }}>{tr(lang, "Зміна паролю", "Set a new password")}</h1>
      {done ? (
        <p className="muted">{tr(lang, "Пароль змінено. Перенаправляємо на вхід…", "Password changed. Redirecting to sign in…")}</p>
      ) : (
        <form className="form" onSubmit={submit}>
          <input className="input" type="password" placeholder={tr(lang, "Новий пароль (мін. 8 символів)", "New password (min. 8 characters)")} value={password} onChange={(e) => setPassword(e.target.value)} required minLength={8} autoComplete="new-password" />
          <input className="input" type="password" placeholder={tr(lang, "Повторіть пароль", "Confirm password")} value={confirm} onChange={(e) => setConfirm(e.target.value)} required autoComplete="new-password" />
          {mismatch && <div className="muted" style={{ fontSize: 13 }}>{tr(lang, "Паролі не співпадають", "Passwords don't match")}</div>}
          {error && <div className="error">{error}</div>}
          <button className="btn block" disabled={busy || password.length < 8 || password !== confirm}>{busy ? "…" : tr(lang, "Змінити пароль", "Change password")}</button>
        </form>
      )}
    </main>
  );
}

export default function ResetPasswordPage() {
  return (
    <Suspense fallback={<main className="container narrow"><p className="muted">…</p></main>}>
      <ResetForm />
    </Suspense>
  );
}
