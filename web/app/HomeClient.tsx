"use client";

import { useCallback, useEffect, useState } from "react";
import EventCard from "./components/EventCard";
import {
  getEvents,
  discoverSections,
  mySaves,
  saveEvent,
  unsaveEvent,
  getToken,
  errorMessage,
  logTelemetry,
  type EventDto,
  type DiscoverSection,
} from "../lib/api";
import { useLang, tr } from "../lib/i18n";

type Mode = "foryou" | "all";

export default function HomeClient() {
  const { lang } = useLang();
  const authed = typeof window !== "undefined" && !!getToken();
  const [mode, setMode] = useState<Mode>("foryou");
  const [events, setEvents] = useState<EventDto[]>([]);
  const [sections, setSections] = useState<DiscoverSection[]>([]);
  const [saved, setSaved] = useState<Set<string>>(new Set());
  const [q, setQ] = useState("");
  const [cat, setCat] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const load = useCallback((m: Mode) => {
    setLoading(true);
    setError(null);
    if (m === "foryou") {
      discoverSections()
        .then((secs) => {
          setSections(secs);
          logTelemetry("discover_impression", {
            sections: secs.length,
            count: secs.reduce((n, s) => n + s.events.length, 0),
          });
        })
        .catch((e) => setError(errorMessage(e)))
        .finally(() => setLoading(false));
    } else {
      getEvents()
        .then(setEvents)
        .catch((e) => setError(errorMessage(e)))
        .finally(() => setLoading(false));
    }
  }, []);

  useEffect(() => load(mode), [load, mode]);
  useEffect(() => {
    if (authed) mySaves().then((list) => setSaved(new Set(list.map((e) => e.id)))).catch(() => {});
  }, [authed]);

  function toggleSave(id: string) {
    const nowSaved = !saved.has(id);
    setSaved((prev) => {
      const next = new Set(prev);
      if (nowSaved) next.add(id);
      else next.delete(id);
      return next;
    });
    (nowSaved ? saveEvent(id) : unsaveEvent(id)).catch(() =>
      setSaved((prev) => {
        const next = new Set(prev);
        if (nowSaved) next.delete(id);
        else next.add(id);
        return next;
      }),
    );
  }

  const categories = Array.from(new Set(events.map((e) => e.category)));
  const shown = events.filter((e) => {
    if (cat && e.category !== cat) return false;
    if (q && !e.title.toLowerCase().includes(q.toLowerCase())) return false;
    return true;
  });
  const card = (e: EventDto) => (
    <EventCard key={e.id} event={e} saved={saved.has(e.id)} onToggleSave={authed ? toggleSave : undefined} />
  );

  return (
    <main className="container">
      <h1 style={{ fontSize: 26, margin: "8px 0 14px" }}>{tr(lang, "Афіша подій", "Events")}</h1>
      <div className="filters">
        <button className={`chip ${mode === "foryou" ? "active" : ""}`} onClick={() => setMode("foryou")}>{tr(lang, "Для тебе", "For you")}</button>
        <button className={`chip ${mode === "all" ? "active" : ""}`} onClick={() => setMode("all")}>{tr(lang, "Усі події", "All events")}</button>
      </div>

      {mode === "all" && (
        <>
          <input className="input" placeholder={tr(lang, "Пошук подій…", "Search events…")} value={q} onChange={(e) => setQ(e.target.value)} style={{ marginBottom: 12 }} />
          <div className="filters">
            <button className={`chip ${cat === "" ? "active" : ""}`} onClick={() => setCat("")}>{tr(lang, "Усі категорії", "All categories")}</button>
            {categories.map((c) => (
              <button key={c} className={`chip ${cat === c ? "active" : ""}`} onClick={() => setCat(cat === c ? "" : c)}>{c}</button>
            ))}
          </div>
        </>
      )}

      {error && <div className="error">{error}</div>}
      {loading && <p className="muted">{tr(lang, "Завантаження…", "Loading…")}</p>}
      {!loading && ((mode === "foryou" && sections.length === 0) || (mode === "all" && shown.length === 0)) && (
        <p className="muted">
          {mode === "foryou"
            ? tr(lang, "Поки нічого порекомендувати. Увійди й обери інтереси, або глянь «Усі події».", "Nothing to recommend yet. Sign in and pick your interests, or browse “All events”.")
            : tr(lang, "Подій не знайдено.", "No events found.")}
        </p>
      )}
      {!loading && mode === "foryou" &&
        sections.map((sec) => (
          <section key={sec.key}>
            <h2 style={{ fontSize: 18, margin: "18px 0 10px" }}>{sec.title}</h2>
            <div className="grid">{sec.events.map(card)}</div>
          </section>
        ))}
      {mode === "all" && <div className="grid">{shown.map(card)}</div>}
    </main>
  );
}
