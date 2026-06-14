"use client";

import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import CityInput from "../components/CityInput";
import { EVENT_CATEGORIES, putPrefs, getPrefs, getToken, errorMessage, safeNext } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";

function OnboardingInner() {
  const router = useRouter();
  const { lang } = useLang();
  const next = safeNext(useSearchParams().get("next"));
  const [selected, setSelected] = useState<string[]>([]);
  const [city, setCity] = useState("Київ");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!getToken()) {
      router.push("/login");
      return;
    }
    // Already-onboarded users (returning, or who picked interests before) skip straight through —
    // onboarding is a one-time step, not a screen you can get stuck on by re-visiting the URL.
    getPrefs()
      .then((p) => {
        if (p.onboarded) router.replace(next);
      })
      .catch(() => {});
  }, [router, next]);

  function toggle(c: string) {
    setSelected((s) => (s.includes(c) ? s.filter((x) => x !== c) : [...s, c]));
  }

  async function finish() {
    setBusy(true);
    setError(null);
    try {
      await putPrefs(selected, city.trim() || null);
      router.push(next);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function skip() {
    // Persist an (empty) onboarding so the guard above won't bounce the user back here later.
    setBusy(true);
    try {
      await putPrefs([], null);
    } catch {
      /* non-fatal: still let them through */
    }
    router.push(next);
  }

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 26 }}>{tr(lang, "Ласкаво просимо!", "Welcome!")}</h1>
      <p className="muted">{tr(lang, "Оберіть інтереси й місто — підберемо події саме для вас.", "Pick interests and your city — we'll tailor events to you.")}</p>

      <h2 style={{ fontSize: 16, marginTop: 16 }}>{tr(lang, "Інтереси", "Interests")}</h2>
      <div className="row gap" style={{ flexWrap: "wrap", marginTop: 6 }}>
        {EVENT_CATEGORIES.map((c) => (
          <button
            key={c}
            type="button"
            className={selected.includes(c) ? "pill ok" : "pill"}
            style={{ cursor: "pointer" }}
            onClick={() => toggle(c)}
          >
            {c}
          </button>
        ))}
      </div>

      <label className="lbl" style={{ marginTop: 16 }}>
        {tr(lang, "Місто", "City")}
        <CityInput value={city} onChange={setCity} />
      </label>

      {error && <div className="error">{error}</div>}
      <button className="btn block" onClick={finish} disabled={busy} style={{ marginTop: 16 }}>
        {busy ? "…" : tr(lang, "Готово", "Done")}
      </button>
      <button className="btn ghost sm" onClick={skip} disabled={busy} style={{ marginTop: 8 }}>
        {tr(lang, "Пропустити", "Skip")}
      </button>
    </main>
  );
}

export default function OnboardingPage() {
  return (
    <Suspense fallback={<main className="container narrow"><p className="muted">…</p></main>}>
      <OnboardingInner />
    </Suspense>
  );
}
