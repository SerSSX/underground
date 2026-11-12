"use client";

import { Suspense, useState, type FormEvent } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { register, setToken, errorMessage, safeNext } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";
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

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      setToken(await register(email.trim(), password, name.trim()));
      refresh();
      // New accounts pick interests and a city first, then go on to where they were headed.
      router.push(`/onboarding?next=${encodeURIComponent(next)}`);
    } catch (err) {
      setError(errorMessage(err));
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
          <label htmlFor="email">Email</label>
          <input id="email" className="input" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required autoComplete="email" inputMode="email" />
        </div>
        <div className="field">
          <label htmlFor="pw">{tr(lang, "Пароль", "Password")}</label>
          <div style={{ position: "relative" }}>
            <input id="pw" className="input" type={show ? "text" : "password"} value={password} onChange={(e) => setPassword(e.target.value)} required minLength={8} autoComplete="new-password" aria-invalid={short} aria-describedby="pw-hint" style={{ paddingRight: 48 }} />
            <button type="button" className="btn quiet icon sm" style={{ position: "absolute", right: 6, top: 6 }} onClick={() => setShow((s) => !s)} aria-label={show ? tr(lang, "Сховати пароль", "Hide password") : tr(lang, "Показати пароль", "Show password")} aria-pressed={show}><Eye className="ico sm" /></button>
          </div>
          <span id="pw-hint" className={short ? "err" : "hint"}>{tr(lang, "Щонайменше 8 символів", "At least 8 characters")}</span>
        </div>
        {error && <div className="alert error">{error}</div>}
        <button className="btn block lg" disabled={busy || !name.trim() || !email.trim() || password.length < 8}>{busy ? "…" : tr(lang, "Створити акаунт", "Create account")}</button>
        <p className="subtle xs" style={{ textAlign: "center" }}>
          {tr(lang, "Реєструючись, ви погоджуєтесь з ", "By signing up you accept the ")}<Link className="link" href="/terms">{tr(lang, "умовами", "terms")}</Link>{tr(lang, " і ", " and ")}<Link className="link" href="/privacy">{tr(lang, "політикою конфіденційності", "privacy policy")}</Link>.
        </p>
      </form>
    </AuthCard>
  );
}

export default function RegisterPage() {
  return <Suspense fallback={null}><RegisterForm /></Suspense>;
}
