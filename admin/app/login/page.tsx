"use client";

import { useState, type FormEvent } from "react";
import { useRouter } from "next/navigation";
import { login, setToken, errorMessage } from "../../lib/api";

export default function LoginPage() {
  const router = useRouter();
  const [email, setEmail] = useState("admin@underground.dev");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const token = await login(email, password);
      setToken(token);
      router.push("/dashboard");
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="card">
      <h1 className="brand">UNDERGROUND</h1>
      <p className="muted" style={{ textAlign: "center", marginTop: 4 }}>Адмін-панель · лише для персоналу</p>
      <form className="form" onSubmit={submit}>
        <label className="lbl">
          Email <span style={{ color: "var(--danger)" }}>*</span>
          <input
            className="input"
            type="email"
            placeholder="you@underground.app"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            autoComplete="email"
            autoFocus
          />
        </label>
        <label className="lbl">
          Пароль <span style={{ color: "var(--danger)" }}>*</span>
          <input
            className="input"
            type="password"
            placeholder="••••••••"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            autoComplete="current-password"
          />
        </label>
        {error && <div className="error">{error}</div>}
        <button className="btn block" disabled={busy || !email.trim() || !password}>{busy ? "Вхід…" : "Увійти"}</button>
      </form>
    </main>
  );
}
