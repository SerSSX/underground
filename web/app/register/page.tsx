"use client";

import { Suspense, useState, type FormEvent } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { register, setToken, errorMessage, safeNext } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";

function RegisterForm() {
  const router = useRouter();
  const { lang } = useLang();
  const next = safeNext(useSearchParams().get("next"));
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      setToken(await register(email.trim(), password, name.trim()));
      // New accounts go through onboarding first, which then forwards to `next`.
      router.push(`/onboarding?next=${encodeURIComponent(next)}`);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24 }}>{tr(lang, "Реєстрація", "Sign up")}</h1>
      <form className="form" onSubmit={submit}>
        <input className="input" placeholder={tr(lang, "Ім'я", "Name")} value={name} onChange={(e) => setName(e.target.value)} required autoComplete="name" />
        <input className="input" type="email" placeholder="Email" value={email} onChange={(e) => setEmail(e.target.value)} required autoComplete="email" />
        <input className="input" type="password" placeholder={tr(lang, "Пароль (мін. 8 символів)", "Password (min. 8 characters)")} value={password} onChange={(e) => setPassword(e.target.value)} required minLength={8} autoComplete="new-password" />
        {password.length > 0 && password.length < 8 && <div className="muted" style={{ fontSize: 13 }}>{tr(lang, "Пароль — щонайменше 8 символів", "Password must be at least 8 characters")}</div>}
        {error && <div className="error">{error}</div>}
        <button className="btn block" disabled={busy || !name.trim() || !email.trim() || password.length < 8}>{busy ? "…" : tr(lang, "Створити акаунт", "Create account")}</button>
      </form>
      <p className="muted" style={{ marginTop: 14 }}>
        {tr(lang, "Уже є акаунт?", "Already have an account?")} <Link className="link" href="/login">{tr(lang, "Увійти", "Log in")}</Link>
      </p>
    </main>
  );
}

export default function RegisterPage() {
  return (
    <Suspense fallback={<main className="container narrow"><p className="muted">…</p></main>}>
      <RegisterForm />
    </Suspense>
  );
}
