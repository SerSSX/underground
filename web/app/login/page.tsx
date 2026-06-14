"use client";

import { Suspense, useState, type FormEvent } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { login, setToken, errorMessage, safeNext } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";

function LoginForm() {
  const router = useRouter();
  const { lang } = useLang();
  const next = safeNext(useSearchParams().get("next"));
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      setToken(await login(email.trim(), password));
      router.push(next);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24 }}>{tr(lang, "Вхід", "Sign in")}</h1>
      <form className="form" onSubmit={submit}>
        <input className="input" type="email" placeholder="Email" value={email} onChange={(e) => setEmail(e.target.value)} required autoComplete="email" />
        <input className="input" type="password" placeholder={tr(lang, "Пароль", "Password")} value={password} onChange={(e) => setPassword(e.target.value)} required autoComplete="current-password" />
        {error && <div className="error">{error}</div>}
        <button className="btn block" disabled={busy || !email.trim() || !password}>{busy ? tr(lang, "Вхід…", "Signing in…") : tr(lang, "Увійти", "Sign in")}</button>
      </form>
      <p className="muted" style={{ marginTop: 14 }}>
        {tr(lang, "Немає акаунта?", "Don't have an account?")} <Link className="link" href={`/register?next=${encodeURIComponent(next)}`}>{tr(lang, "Зареєструватися", "Sign up")}</Link>
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
