"use client";

import { Suspense, useState, type FormEvent } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { login, requestEmailCode, verifyEmailCode, setToken, errorMessage, safeNext } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";

function LoginForm() {
  const router = useRouter();
  const { lang } = useLang();
  const next = safeNext(useSearchParams().get("next"));
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [codeSent, setCodeSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function run(fn: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  const submitPassword = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => {
      setToken(await login(email.trim(), password));
      router.push(next);
    });
  };
  const sendCode = () =>
    run(async () => {
      await requestEmailCode(email.trim());
      setCodeSent(true);
    });
  const submitCode = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => {
      setToken(await verifyEmailCode(email.trim(), code.trim()));
      router.push(next);
    });
  };

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24 }}>{tr(lang, "Вхід", "Sign in")}</h1>
      {error && <div className="error">{error}</div>}

      {!codeSent ? (
        <>
          <form className="form" onSubmit={submitPassword}>
            <input className="input" type="email" placeholder="Email" value={email} onChange={(e) => setEmail(e.target.value)} required autoComplete="email" />
            <input className="input" type="password" placeholder={tr(lang, "Пароль", "Password")} value={password} onChange={(e) => setPassword(e.target.value)} required autoComplete="current-password" />
            <button className="btn block" disabled={busy || !email.trim() || !password}>
              {busy ? tr(lang, "Вхід…", "Signing in…") : tr(lang, "Увійти", "Sign in")}
            </button>
          </form>

          <div className="row gap" style={{ alignItems: "center", margin: "14px 0", color: "var(--muted)" }}>
            <div style={{ flex: 1, height: 1, background: "var(--border)" }} />
            <span style={{ fontSize: 12 }}>{tr(lang, "або", "or")}</span>
            <div style={{ flex: 1, height: 1, background: "var(--border)" }} />
          </div>
          <button type="button" className="btn ghost block" disabled={busy || !email.trim()} onClick={sendCode}>
            {tr(lang, "Увійти кодом на email", "Sign in with an email code")}
          </button>
          <p className="muted" style={{ fontSize: 12, marginTop: 6 }}>
            {tr(lang, "Введи email вище — надішлемо одноразовий код.", "Enter your email above — we'll send a one-time code.")}
          </p>
        </>
      ) : (
        <form className="form" onSubmit={submitCode}>
          <p className="muted" style={{ fontSize: 13 }}>{tr(lang, `Код надіслано на ${email}.`, `Code sent to ${email}.`)}</p>
          <input className="input" inputMode="numeric" placeholder={tr(lang, "Код з email", "Code from email")} value={code} onChange={(e) => setCode(e.target.value)} required autoFocus />
          <button className="btn block" disabled={busy || !code.trim()}>
            {busy ? tr(lang, "Вхід…", "Signing in…") : tr(lang, "Увійти", "Sign in")}
          </button>
          <button type="button" className="btn ghost" onClick={() => { setCodeSent(false); setCode(""); }}>
            {tr(lang, "Назад", "Back")}
          </button>
        </form>
      )}

      <p className="muted" style={{ marginTop: 14 }}>
        {tr(lang, "Немає акаунта?", "Don't have an account?")}{" "}
        <Link className="link" href={`/register?next=${encodeURIComponent(next)}`}>{tr(lang, "Зареєструватися", "Sign up")}</Link>
      </p>
      <p className="muted" style={{ marginTop: 6 }}>
        <Link className="link" href="/forgot-password">{tr(lang, "Забули пароль?", "Forgot password?")}</Link>
      </p>
    </main>
  );
}

export default function LoginPage() {
  return (
    <Suspense fallback={<main className="container narrow"><p className="muted">…</p></main>}>
      <LoginForm />
    </Suspense>
  );
}
