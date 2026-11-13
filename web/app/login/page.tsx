"use client";

import { Suspense, useState, type FormEvent } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { login, requestEmailCode, verifyEmailCode, setToken, errorMessage, isUnauthorized, safeNext } from "../../lib/api";
import { EMAIL_CODE_LOGIN } from "../../lib/flags";
import { useLang, tr, tf } from "../../lib/i18n";
import { useSession } from "../../lib/session";
import AuthCard from "../components/AuthCard";
import { Eye } from "../components/Icon";

function LoginForm() {
  const router = useRouter();
  const { lang } = useLang();
  const { refresh } = useSession();
  const next = safeNext(useSearchParams().get("next"));
  const [mode, setMode] = useState<"password" | "code">("password");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [show, setShow] = useState(false);
  const [code, setCode] = useState("");
  const [codeSent, setCodeSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function run(fn: () => Promise<void>, wrong?: string) {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (err) {
      // A 401 here means the credentials were wrong, not that a sign-in is needed.
      setError(wrong && isUnauthorized(err) ? wrong : errorMessage(err));
      setBusy(false);
    }
  }
  const done = (token: string) => {
    setToken(token);
    refresh();
    router.push(next);
  };

  const submitPassword = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => done(await login(email.trim(), password)), tr(lang, "Невірний email або пароль", "Wrong email or password"));
  };
  const sendCode = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => {
      await requestEmailCode(email.trim());
      setCodeSent(true);
      setBusy(false);
    });
  };
  const submitCode = (e: FormEvent) => {
    e.preventDefault();
    void run(async () => done(await verifyEmailCode(email.trim(), code.trim())), tr(lang, "Невірний або прострочений код", "Wrong or expired code"));
  };

  return (
    <AuthCard
      title={tr(lang, "З поверненням", "Welcome back")}
      subtitle={tr(lang, "Увійдіть, щоб бронювати місця й створювати події.", "Sign in to book places and create events.")}
      footer={<>{tr(lang, "Ще немає акаунта?", "No account yet?")} <Link className="link" href={`/register?next=${encodeURIComponent(next)}`}>{tr(lang, "Зареєструватися", "Sign up")}</Link></>}
    >
      {EMAIL_CODE_LOGIN && (
        <div className="seg" style={{ width: "100%", marginBottom: 18 }}>
          <button type="button" className={mode === "password" ? "active" : ""} style={{ flex: 1 }} onClick={() => setMode("password")}>{tr(lang, "Пароль", "Password")}</button>
          <button type="button" className={mode === "code" ? "active" : ""} style={{ flex: 1 }} onClick={() => setMode("code")}>{tr(lang, "Код на пошту", "Email code")}</button>
        </div>
      )}
      {error && <div className="alert error" style={{ marginBottom: 14 }}>{error}</div>}

      {mode === "password" ? (
        <form className="form" onSubmit={submitPassword}>
          <div className="field">
            <label htmlFor="email">Email</label>
            <input id="email" className="input" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required autoComplete="email" inputMode="email" />
          </div>
          <div className="field">
            <div className="row spread"><label htmlFor="pw">{tr(lang, "Пароль", "Password")}</label>{EMAIL_CODE_LOGIN && <Link className="link small" href="/forgot-password">{tr(lang, "Забули?", "Forgot?")}</Link>}</div>
            <div style={{ position: "relative" }}>
              <input id="pw" className="input" type={show ? "text" : "password"} value={password} onChange={(e) => setPassword(e.target.value)} required autoComplete="current-password" style={{ paddingRight: 48 }} />
              <button type="button" className="btn quiet icon sm" style={{ position: "absolute", right: 6, top: 6 }} onClick={() => setShow((s) => !s)} aria-label={show ? tr(lang, "Сховати пароль", "Hide password") : tr(lang, "Показати пароль", "Show password")} aria-pressed={show}><Eye className="ico sm" /></button>
            </div>
          </div>
          <button className="btn block lg" disabled={busy || !email.trim() || !password}>{busy ? tr(lang, "Входимо…", "Signing in…") : tr(lang, "Увійти", "Sign in")}</button>
        </form>
      ) : !codeSent ? (
        <form className="form" onSubmit={sendCode}>
          <div className="field">
            <label htmlFor="email2">Email</label>
            <input id="email2" className="input" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required autoComplete="email" inputMode="email" />
            <span className="hint">{tr(lang, "Надішлемо 6-значний код. Пароль не потрібен.", "We'll send a 6-digit code. No password needed.")}</span>
          </div>
          <button className="btn block lg" disabled={busy || !email.trim()}>{tr(lang, "Отримати код", "Send code")}</button>
        </form>
      ) : (
        <form className="form" onSubmit={submitCode}>
          <div className="alert ok">{tf(lang, "Код надіслано на {email}", "Code sent to {email}", { email })}</div>
          <div className="field">
            <label htmlFor="code">{tr(lang, "Код з листа", "Code from the email")}</label>
            <input id="code" className="input" inputMode="numeric" autoComplete="one-time-code" maxLength={6} value={code} onChange={(e) => setCode(e.target.value.replace(/\D/g, ""))} required style={{ letterSpacing: "0.4em", fontSize: 22, textAlign: "center" }} />
          </div>
          <button className="btn block lg" disabled={busy || code.length !== 6}>{tr(lang, "Увійти", "Sign in")}</button>
          <button type="button" className="btn quiet block" onClick={() => { setCodeSent(false); setCode(""); }}>{tr(lang, "Інша адреса", "Different email")}</button>
        </form>
      )}
    </AuthCard>
  );
}

export default function LoginPage() {
  return <Suspense fallback={null}><LoginForm /></Suspense>;
}
