"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useState } from "react";
import EventCard, { EventCardSkeleton } from "./components/EventCard";
import { getEvents, discoverSections, mySaves, saveEvent, unsaveEvent, getToken, errorMessage, logTelemetry, type EventDto, type DiscoverSection } from "../lib/api";
import { useLang, tr } from "../lib/i18n";
import { categoryLabel, inWindow, TOP_CATEGORIES, type When } from "../lib/ui";
import { Search, MapPin, Plus, Compass } from "./components/Icon";

const CITY_KEY = "ug_city";

export default function HomeClient() {
  const { lang } = useLang();
  const router = useRouter();
  const [events, setEvents] = useState<EventDto[] | null>(null);
  const [sections, setSections] = useState<DiscoverSection[]>([]);
  const [saved, setSaved] = useState<Set<string>>(new Set());
  const [q, setQ] = useState("");
  const [city, setCity] = useState("");
  const [when, setWhen] = useState<When>("all");
  const [cat, setCat] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    try { setCity(localStorage.getItem(CITY_KEY) ?? ""); } catch { /* storage blocked */ }
    getEvents().then(setEvents).catch((e) => { setError(errorMessage(e)); setEvents([]); });
    discoverSections()
      .then((secs) => {
        setSections(secs.filter((s) => s.events.length > 0));
        logTelemetry("discover_impression", { sections: secs.length, count: secs.reduce((n, s) => n + s.events.length, 0) });
      })
      .catch(() => {});
    if (getToken()) mySaves().then((l) => setSaved(new Set(l.map((e) => e.id)))).catch(() => {});
  }, []);

  function chooseCity(c: string) {
    setCity(c);
    try { if (c) localStorage.setItem(CITY_KEY, c); else localStorage.removeItem(CITY_KEY); } catch { /* ignore */ }
  }

  function toggleSave(id: string) {
    if (!getToken()) {
      router.push(`/login?next=/`);
      return;
    }
    const now = !saved.has(id);
    const flip = (on: boolean) => setSaved((prev) => { const n = new Set(prev); if (on) n.add(id); else n.delete(id); return n; });
    flip(now);
    (now ? saveEvent(id) : unsaveEvent(id)).catch(() => flip(!now));
  }

  const cities = useMemo(() => {
    const count = new Map<string, number>();
    for (const e of events ?? []) if (e.city) count.set(e.city, (count.get(e.city) ?? 0) + 1);
    return [...count.entries()].sort((a, b) => b[1] - a[1]).map(([c]) => c);
  }, [events]);

  const categories = useMemo(() => {
    const present = new Set((events ?? []).map((e) => e.category.trim().toLowerCase()));
    const top = TOP_CATEGORIES.filter((c) => present.has(c));
    const rest = [...present].filter((c) => !TOP_CATEGORIES.includes(c)).sort((a, b) => categoryLabel(a, lang).localeCompare(categoryLabel(b, lang)));
    return [...top, ...rest];
  }, [events, lang]);

  // A remembered city with no events right now would filter to nothing while the select shows "all".
  useEffect(() => { if (city && events && !cities.includes(city)) chooseCity(""); }, [city, cities, events]);

  const filtering = !!q.trim() || !!cat || when !== "all";
  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return (events ?? []).filter((e) => {
      if (city && e.city !== city) return false;
      if (cat && e.category.trim().toLowerCase() !== cat) return false;
      if (!inWindow(e.starts_at, when, new Date(), e.timezone)) return false;
      if (needle && !`${e.title} ${e.description ?? ""} ${e.venue_address ?? ""}`.toLowerCase().includes(needle)) return false;
      return true;
    });
  }, [events, q, city, cat, when]);

  // When a date window is empty, offer the next events that match everything else.
  const nearest = useMemo(() => {
    if (when === "all") return [];
    const needle = q.trim().toLowerCase();
    return (events ?? [])
      .filter((e) => (!city || e.city === city) && (!cat || e.category.trim().toLowerCase() === cat))
      .filter((e) => !needle || `${e.title} ${e.description ?? ""}`.toLowerCase().includes(needle))
      .filter((e) => e.starts_at && Date.parse(e.starts_at) > Date.now())
      .slice(0, 4);
  }, [events, when, city, cat, q]);

  const card = (e: EventDto, i: number) => (
    <EventCard key={e.id} event={e} saved={saved.has(e.id)} onToggleSave={toggleSave} eager={i < 4} />
  );
  const WHEN: [When, string, string][] = [
    ["all", "Усі дати", "Any date"], ["today", "Сьогодні", "Today"], ["tomorrow", "Завтра", "Tomorrow"],
    ["weekend", "Вихідні", "Weekend"], ["week", "Найближчі 7 днів", "Next 7 days"],
  ];
  const sectionsForCity = sections
    .map((s) => ({ ...s, events: city ? s.events.filter((e) => e.city === city) : s.events }))
    .filter((s) => s.events.length > 0);

  return (
    <main className="container">
      <section className="hero-search">
        <h1>{tr(lang, "Куди піти ", "Where to go ")}<em>{tr(lang, "сьогодні?", "tonight?")}</em></h1>
        <p>{tr(lang, "Концерти, вечірки, лекції й зустрічі поруч. Бронюй місця безкоштовно — квиток з QR одразу в телефоні.", "Concerts, parties, talks and meetups near you. Book free places — a QR ticket lands on your phone at once.")}</p>
        <div className="searchbar" role="search">
          <label className="input-icon">
            <span className="sr-only">{tr(lang, "Пошук подій", "Search events")}</span>
            <Search className="ico" />
            <input className="input" type="search" placeholder={tr(lang, "Назва, місце або виконавець", "Title, venue or artist")} value={q} onChange={(e) => setQ(e.target.value)} />
          </label>
          <label className="input-icon city">
            <span className="sr-only">{tr(lang, "Місто", "City")}</span>
            <MapPin className="ico" />
            <select className="input" value={city} onChange={(e) => chooseCity(e.target.value)}>
              <option value="">{tr(lang, "Усі міста", "All cities")}</option>
              {cities.map((c) => <option key={c} value={c}>{c}</option>)}
            </select>
          </label>
        </div>
        <div className="filter-row">
          <div className="chips scroll" role="group" aria-label={tr(lang, "Коли", "When")}>
            {WHEN.map(([k, uk, en]) => (
              <button key={k} className={`chip ${when === k ? "active" : ""}`} aria-pressed={when === k} onClick={() => setWhen(k)}>{tr(lang, uk, en)}</button>
            ))}
          </div>
        </div>
        <div className="filter-row" style={{ marginTop: 10 }}>
          <div className="chips scroll" role="group" aria-label={tr(lang, "Категорії", "Categories")}>
            <button className={`chip ${cat === "" ? "active" : ""}`} aria-pressed={cat === ""} onClick={() => setCat("")}>{tr(lang, "Усі категорії", "All categories")}</button>
            {categories.map((c) => (
              <button key={c} className={`chip ${cat === c ? "active" : ""}`} aria-pressed={cat === c} onClick={() => setCat(cat === c ? "" : c)}>{categoryLabel(c, lang)}</button>
            ))}
          </div>
        </div>
      </section>

      {error && <div className="alert error" style={{ marginTop: 20 }}>{error}</div>}

      {!filtering && sectionsForCity.map((sec) => (
        <section key={sec.key} className="section">
          <div className="section-head"><h2>{sec.title}</h2></div>
          <div className="hscroll">{sec.events.map(card)}</div>
        </section>
      ))}

      <section className="section">
        <div className="section-head">
          <h2>{filtering ? tr(lang, "Знайдено", "Results") : city ? tr(lang, `Усі події · ${city}`, `All events · ${city}`) : tr(lang, "Усі події", "All events")}</h2>
          {events && <span className="muted small">{shown.length}</span>}
        </div>
        {events === null ? (
          <div className="grid">{Array.from({ length: 8 }, (_, i) => <EventCardSkeleton key={i} />)}</div>
        ) : shown.length === 0 && when !== "all" && nearest.length > 0 ? (
          <>
            <div className="alert" style={{ marginBottom: 16 }}>{tr(lang, "На ці дати подій немає. Ось найближчі:", "Nothing on those dates. The nearest ones:")}</div>
            <div className="grid">{nearest.map(card)}</div>
          </>
        ) : shown.length === 0 ? (
          <div className="empty">
            <div className="ei"><Compass className="ico lg" /></div>
            <h3>{tr(lang, "Нічого не знайшли", "Nothing found")}</h3>
            <p>{tr(lang, "Спробуйте інші дати чи категорію — або створіть свою подію, і її побачать інші.", "Try other dates or a category — or create your own event for others to find.")}</p>
            <div className="row gap" style={{ justifyContent: "center" }}>
              {(filtering || city) && <button className="btn ghost" onClick={() => { setQ(""); setCat(""); setWhen("all"); chooseCity(""); }}>{tr(lang, "Скинути фільтри", "Reset filters")}</button>}
              <Link className="btn" href="/organizer/new"><Plus className="ico sm" />{tr(lang, "Створити подію", "Create event")}</Link>
            </div>
          </div>
        ) : (
          <div className="grid">{shown.map(card)}</div>
        )}
      </section>
    </main>
  );
}
