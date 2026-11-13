"use client";

import { Suspense, useState, type FormEvent } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { register, setToken, errorMessage, safeNext, ApiError } from "../../lib/api";
import { setPrefillEmail } from "../../lib/prefill";
import { useLang, rich, tr, tf } from "../../lib/i18n";
import { useSession } from "../../lib/session";
import AuthCard from "../components/AuthCard";
import { Eye } from "../components/Icon";

function RegisterForm() {
  const router = useRouter();
  const { lang } = useLang();
  const { refresh } = useSession();
  const next = safeNext(useSearchParams().get("next"));
  const [name, setName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [show, setShow] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // The address already has an account (API 409): offer to sign in with it instead (TASK-304).
  const [exists, setExists] = useState(false);

  const goLogin = () => {
    setPrefillEmail(email.trim());
    router.push(`/login?next=${encodeURIComponent(next)}`);
  };

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setExists(false);
    try {
      setToken(await register(email.trim(), password, name.trim()));
      refresh();
      // Someone who signed up on the way to something (booking, creating an event) goes straight there;
      // interests are asked only of people who came to browse.
      router.push(next === "/" ? `/onboarding?next=${encodeURIComponent(next)}` : next);
    } catch (err) {
      if (err instanceof ApiError && err.status === 409) setExists(true);
      else setError(errorMessage(err));
      setBusy(false);
    }
  }

  const short = password.length > 0 && password.length < 8;
  return (
    <AuthCard
      title={tr(lang, "Створіть акаунт", "Create your account")}
      subtitle={tr(lang, "Бронюйте місця безкоштовно й публікуйте власні події.", "Book free places and publish your own events.")}
      footer={<>{tr(lang, "Уже є акаунт?", "Already have an account?")} <Link className="link" href={`/login?next=${encodeURIComponent(next)}`}>{tr(lang, "Увійти", "Sign in")}</Link></>}
    >
      <form className="form" onSubmit={submit}>
        <div className="field">
          <label htmlFor="name">{tr(lang, "Імʼя", "Name")}</label>
          <input id="name" className="input" value={name} onChange={(e) => setName(e.target.value)} required maxLength={80} autoComplete="name" />
          <span className="hint">{tr(lang, "Його побачить організатор у списку гостей.", "The organiser sees it in the guest list.")}</span>
        </div>
        <div className="field">
          <label htmlFor="email">{tr(lang, "Email", "Email")}</label>
          <input id="email" className="input" type="email" value={email} onChange={(e) => { setEmail(e.target.value); setExists(false); }} required autoComplete="email" inputMode="email" />
        </div>
        <div className="field">
          <label htmlFor="pw">{tr(lang, "Пароль", "Password")}</label>
          <div style={{ position: "relative" }}>
            <input id="pw" className="input" type={show ? "text" : "password"} value={password} onChange={(e) => setPassword(e.target.value)} required minLength={8} autoComplete="new-password" aria-invalid={short} aria-describedby="pw-hint" style={{ paddingRight: 48 }} />
            <button type="button" className="btn quiet icon sm" style={{ position: "absolute", right: 6, top: 6 }} onClick={() => setShow((s) => !s)} aria-label={show ? tr(lang, "Сховати пароль", "Hide password") : tr(lang, "Показати пароль", "Show password")} aria-pressed={show}><Eye className="ico sm" /></button>
          </div>
          <span id="pw-hint" className={short ? "err" : "hint"}>{tr(lang, "Щонайменше 8 символів", "At least 8 characters")}</span>
        </div>
        {exists && (
          <div className="alert" role="alert" style={{ flexDirection: "column", alignItems: "stretch", gap: 10 }}>
            <span>{tr(lang, "Ця пошта вже має акаунт. Увійдіть у нього.", "This email already has an account. Log in to it.")}</span>
            <button type="button" className="btn secondary" onClick={goLogin}>{tr(lang, "Увійти", "Log in")}</button>
          </div>
        )}
        {error && <div className="alert error">{error}</div>}
        <button className="btn block lg" disabled={busy || !name.trim() || !email.trim() || password.length < 8}>{busy ? "…" : tr(lang, "Створити акаунт", "Create account")}</button>
        <p className="subtle xs" style={{ textAlign: "center" }}>
          {/* One sentence per language, links placed by the translation (verb-final languages, "。"). */}
          {rich(tf(lang, "Реєструючись, ви погоджуєтесь з {terms} і {privacy}.", "By signing up you accept the {terms} and the {privacy}.", {}), {
            terms: <Link className="link" href="/terms">{tr(lang, "умовами", "terms")}</Link>,
            privacy: <Link className="link" href="/privacy">{tr(lang, "політикою конфіденційності", "privacy policy")}</Link>,
          })}
        </p>
      </form>
    </AuthCard>
  );
}

export default function RegisterPage() {
  return <Suspense fallback={null}><RegisterForm /></Suspense>;
}
