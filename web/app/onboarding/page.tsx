"use client";

import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import CityInput from "../components/CityInput";
import { parseStoredCity } from "../../lib/geo-core.mjs";
import { EVENT_CATEGORIES, putPrefs, getPrefs, getToken, errorMessage, safeNext } from "../../lib/api";
import { useLang, tr, tf } from "../../lib/i18n";
import { categoryLabel } from "../../lib/ui";
import { Check } from "../components/Icon";

function OnboardingInner() {
  const router = useRouter();
  const { lang } = useLang();
  const next = safeNext(useSearchParams().get("next"));
  const [selected, setSelected] = useState<string[]>([]);
  const [city, setCity] = useState("");
  // The picked (or remembered) city's country, sent with prefs; typing a name drops it.
  const [cc, setCc] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Start from the city chosen on the home page, if any (no fixed default: the app is worldwide).
    try {
      const stored = parseStoredCity(localStorage.getItem("ug_city"));
      if (stored) { setCity((c) => c || stored.name); setCc((x) => x ?? stored.country_code ?? null); }
    } catch { /* storage blocked */ }
  }, []);

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
      await putPrefs(selected, city.trim() || null, cc);
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
      <div className="card" style={{ marginTop: 16, padding: "32px 28px", borderRadius: "var(--r-xl)" }}>
        <div className="muted xs strong" style={{ textTransform: "uppercase", letterSpacing: ".08em" }}>{tr(lang, "Крок 1 з 1", "Step 1 of 1")}</div>
        <h1 style={{ fontSize: 30, marginTop: 8 }}>{tr(lang, "Що вам цікаво?", "What are you into?")}</h1>
        <p className="muted" style={{ marginTop: 8 }}>{tr(lang, "Оберіть кілька тем і своє місто — афіша «Для тебе» підлаштується під вас. Змінити можна будь-коли в профілі.", "Pick a few topics and your city — the For You feed adapts. You can change this any time in your profile.")}</p>

        <div className="chips" style={{ marginTop: 22 }} role="group" aria-label={tr(lang, "Інтереси", "Interests")}>
          {EVENT_CATEGORIES.map((c) => {
            const on = selected.includes(c);
            return (
              <button key={c} type="button" className={`chip ${on ? "active" : ""}`} aria-pressed={on} onClick={() => toggle(c)}>
                <Check className="ico" style={{ opacity: on ? 1 : 0 }} aria-hidden />{categoryLabel(c, lang)}
              </button>
            );
          })}
        </div>

        <div className="field" style={{ marginTop: 24, maxWidth: 360 }}>
          <label htmlFor="city">{tr(lang, "Ваше місто", "Your city")}</label>
          <CityInput id="city" value={city} onChange={(v) => { setCity(v); setCc(null); }} onPick={(p, name) => { setCity(name); setCc(p.country_code); }} />
        </div>

        {error && <div className="alert error" style={{ marginTop: 14 }}>{error}</div>}
        <div className="row gap wrap" style={{ marginTop: 24 }}>
          <button className="btn lg" onClick={finish} disabled={busy}>{busy ? "…" : selected.length ? tf(lang, "Готово · {n}", "Done · {n}", { n: selected.length }) : tr(lang, "Готово", "Done")}</button>
          <button className="btn quiet" onClick={skip} disabled={busy}>{tr(lang, "Пропустити", "Skip")}</button>
        </div>
      </div>
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
