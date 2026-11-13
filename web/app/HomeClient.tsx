"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useMemo, useRef, useState } from "react";
import EventCard, { EventCardSkeleton } from "./components/EventCard";
import { getEvents, discoverSections, mySaves, saveEvent, unsaveEvent, getToken, errorMessage, logTelemetry, searchCities, type EventDto, type DiscoverSection } from "../lib/api";
import { useLang, tr, tf, tp, serverText } from "../lib/i18n";
import { categoryLabel, inWindow, TOP_CATEGORIES, type When } from "../lib/ui";
import { Search, MapPin, Plus, Compass, LocateFixed, X } from "./components/Icon";
import CityInput from "./components/CityInput";
import { reversePlace, searchPlaces } from "../lib/geo";
import { cityName, cityStoredName, trustedCityRow, distanceKm, inCity, parseStoredCity, roundCoord, sortByDistance, storeCity, type ChosenCity, type GeoPlace, type Point } from "../lib/geo-core.mjs";

const CITY_KEY = "ug_city";
/** A city that has events: its name, with an event's point so other spellings of it match too. */
const pointOf = (name: string, e: EventDto): ChosenCity => ({ name, ...(e.lat != null && e.lng != null ? { lat: e.lat, lng: e.lng } : {}) });
/** "Create the first event" here: the city's name and, for its time zone, its point (~11 km, so a near-me
 *  position never travels in a URL more precisely than the city) and country (fix F7). */
function createHereHref(c: ChosenCity): string {
  const p = new URLSearchParams({ city: c.name });
  if (c.lat !== undefined && c.lng !== undefined) {
    p.set("lat", String(roundCoord(c.lat, 1)));
    p.set("lng", String(roundCoord(c.lng, 1)));
  }
  if (c.country_code) p.set("cc", c.country_code);
  return `/organizer/new?${p}`;
}
type NearState = "idle" | "locating" | "denied" | "unavailable" | "timeout" | "unsupported" | "unnamed";

export default function HomeClient() {
  const { lang } = useLang();
  const router = useRouter();
  const [events, setEvents] = useState<EventDto[] | null>(null);
  const [sections, setSections] = useState<DiscoverSection[]>([]);
  const [saved, setSaved] = useState<Set<string>>(new Set());
  const [q, setQ] = useState("");
  // The city filter: chosen from worldwide suggestions, typed, or "near me" (TASK-303 G2).
  const [city, setCity] = useState<ChosenCity | null>(null);
  const [cityText, setCityText] = useState("");
  // The visitor's own position, in memory only (never stored or sent unrounded), for distances.
  const [me, setMe] = useState<Point | null>(null);
  const [near, setNear] = useState<NearState>("idle");
  // Bumped on every change of the chosen city: an answer that arrives after a newer choice is dropped.
  const citySeq = useRef(0);
  const [when, setWhen] = useState<When>("all");
  const [cat, setCat] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    try {
      const stored = parseStoredCity(localStorage.getItem(CITY_KEY));
      if (stored) { setCity(stored); setCityText(stored.label ?? stored.name); }
    } catch { /* storage blocked */ }
    getEvents().then(setEvents).catch((e) => { setError(errorMessage(e)); setEvents([]); });
    discoverSections()
      .then((secs) => {
        setSections(secs.filter((s) => s.events.length > 0));
        logTelemetry("discover_impression", { sections: secs.length, count: secs.reduce((n, s) => n + s.events.length, 0) });
      })
      .catch(() => {});
    if (getToken()) mySaves().then((l) => setSaved(new Set(l.map((e) => e.id)))).catch(() => {});
  }, []);

  function chooseCity(c: ChosenCity | null) {
    citySeq.current++;
    setCity(c);
    setCityText(c?.label ?? c?.name ?? "");
    if (!c?.near) setMe(null);
    try { if (c) localStorage.setItem(CITY_KEY, storeCity(c)); else localStorage.removeItem(CITY_KEY); } catch { /* ignore */ }
  }

  function typeCity(text: string) {
    setCityText(text);
    // Cleared → all cities; a name that has events applies at once; any other name on Enter (commitCity).
    // Filtering on every keystroke would flash "no events in Por" while typing "Porto".
    const name = text.replace(/\s+/g, " ").trim();
    if (!name) return chooseCity(null);
    const known = cities.find(([c]) => c.trim().toLowerCase() === name.toLowerCase());
    if (known) chooseCity(pointOf(known[0], known[1].e));
  }

  /** Enter on a typed name: find its point first (a city suggestion with that name, else our gazetteer), so
   *  "Київ" typed by hand also matches events filed as "Kyiv" nearby; only then apply it (fix F6). */
  async function commitCity() {
    const name = cityText.replace(/\s+/g, " ").trim();
    const lower = name.toLowerCase();
    if (!name || lower === city?.name.toLowerCase() || lower === city?.label?.toLowerCase()) return;
    const seq = citySeq.current;
    let chosen: ChosenCity = { name };
    try {
      const same = (p: GeoPlace) => [cityName(p), cityStoredName(p), p.name].some((n) => !!n && n.toLowerCase() === lower);
      const hit = name.length >= 2 ? (await searchPlaces(name, "city", lang)).find(same) : undefined;
      if (hit) {
        chosen = { name: cityStoredName(hit) || name, label: cityName(hit) || undefined, lat: hit.lat, lng: hit.lng, country_code: hit.country_code ?? undefined };
      } else {
        const row = trustedCityRow(await searchCities(name), name, null);
        if (row) chosen = { name: row.name, lat: row.lat, lng: row.lng, country_code: row.country_code ?? undefined };
      }
    } catch { /* lookups failed: filter by the name alone */ }
    if (seq !== citySeq.current) return; // the visitor chose something else meanwhile
    chooseCity(chosen);
  }

  /** "Near me": only on this click; the position is named by the API (sent rounded to ~1 km). */
  function nearMe() {
    if (typeof navigator === "undefined" || !navigator.geolocation) return setNear("unsupported");
    setNear("locating");
    // The city at the click: if the visitor picks another before the answer comes, the answer is dropped (fix F9).
    const at = citySeq.current;
    const stale = () => citySeq.current !== at;
    navigator.geolocation.getCurrentPosition(
      async (pos) => {
        if (stale()) return setNear("idle");
        const here = { lat: pos.coords.latitude, lng: pos.coords.longitude };
        try {
          const place = await reversePlace(here.lat, here.lng, lang);
          if (stale()) return setNear("idle");
          const label = cityName(place) || place?.name || "";
          const name = cityStoredName(place) || label;
          if (!place || !name) return setNear("unnamed");
          chooseCity({ name, label: label !== name ? label : undefined, lat: here.lat, lng: here.lng, country_code: place.country_code ?? undefined, near: true });
          setMe(here);
          setNear("idle");
        } catch {
          setNear(stale() ? "idle" : "unnamed");
        }
      },
      (err) => stale() ? setNear("idle") : setNear(err.code === err.PERMISSION_DENIED ? "denied" : err.code === err.TIMEOUT ? "timeout" : "unavailable"),
      { timeout: 10_000, maximumAge: 300_000, enableHighAccuracy: false },
    );
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

  // Cities that have events, busiest first: offered on an empty city field.
  const cities = useMemo(() => {
    const count = new Map<string, { n: number; e: EventDto }>();
    for (const e of events ?? []) if (e.city) count.set(e.city, { n: (count.get(e.city)?.n ?? 0) + 1, e });
    return [...count.entries()].sort((a, b) => b[1].n - a[1].n).slice(0, 8);
  }, [events]);

  const categories = useMemo(() => {
    const present = new Set((events ?? []).map((e) => e.category.trim().toLowerCase()));
    const top = TOP_CATEGORIES.filter((c) => present.has(c));
    const rest = [...present].filter((c) => !TOP_CATEGORIES.includes(c)).sort((a, b) => categoryLabel(a, lang).localeCompare(categoryLabel(b, lang)));
    return [...top, ...rest];
  }, [events, lang]);

  // Distances: from the visitor after "near me", else from the remembered near-me point.
  const origin: Point | null = me ?? (city?.near && city.lat !== undefined && city.lng !== undefined ? { lat: city.lat, lng: city.lng } : null);
  const byDistance = (list: EventDto[]) => (origin ? sortByDistance(list, origin) : list);

  const filtering = !!q.trim() || !!cat || when !== "all";
  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return byDistance((events ?? []).filter((e) => {
      if (!inCity(e, city)) return false;
      if (cat && e.category.trim().toLowerCase() !== cat) return false;
      if (!inWindow(e.starts_at, when, new Date(), e.timezone)) return false;
      if (needle && !`${e.title} ${e.description ?? ""} ${e.venue_address ?? ""}`.toLowerCase().includes(needle)) return false;
      return true;
    }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [events, q, city, cat, when, me]);
  const cityHasEvents = !city || (events ?? []).some((e) => inCity(e, city));

  // When a date window is empty, offer the next events that match everything else.
  const nearest = useMemo(() => {
    if (when === "all") return [];
    const needle = q.trim().toLowerCase();
    return (events ?? [])
      .filter((e) => inCity(e, city) && (!cat || e.category.trim().toLowerCase() === cat))
      .filter((e) => !needle || `${e.title} ${e.description ?? ""} ${e.venue_address ?? ""}`.toLowerCase().includes(needle))
      .filter((e) => e.starts_at && Date.parse(e.starts_at) > Date.now())
      .slice(0, 4);
  }, [events, when, city, cat, q]);

  const card = (e: EventDto, i: number) => (
    <EventCard key={e.id} event={e} saved={saved.has(e.id)} onToggleSave={toggleSave} eager={i < 4} distanceKm={origin ? distanceKm(e, origin) : null} />
  );
  const WHEN: [When, string][] = [
    ["all", tr(lang, "Усі дати", "Any date")], ["today", tr(lang, "Сьогодні", "Today")], ["tomorrow", tr(lang, "Завтра", "Tomorrow")],
    ["weekend", tr(lang, "Вихідні", "Weekend")], ["week", tr(lang, "Найближчі 7 днів", "Next 7 days")],
  ];
  const sectionsForCity = sections
    .map((s) => ({ ...s, events: s.events.filter((e) => inCity(e, city)) }))
    .filter((s) => s.events.length > 0);

  return (
    <main className="container">
      <section className="hero-search">
        <h1><em>{tr(lang, "Куди піти сьогодні?", "Where to go tonight?")}</em></h1>
        <p>{tr(lang, "Концерти, вечірки, лекції й зустрічі поруч. Бронюй місця безкоштовно — квиток з QR одразу в телефоні.", "Concerts, parties, talks and meetups near you. Book free places — a QR ticket lands on your phone at once.")}</p>
        <div className="searchbar" role="search">
          <label className="input-icon">
            <span className="sr-only">{tr(lang, "Пошук подій", "Search events")}</span>
            <Search className="ico" />
            <input className="input" type="search" placeholder={tr(lang, "Назва, місце або виконавець", "Title, venue or artist")} value={q} onChange={(e) => setQ(e.target.value)} />
          </label>
          <div className="input-icon city">
            <MapPin className="ico" />
            <CityInput
              value={cityText}
              onChange={typeCity}
              onPick={(p, name) => {
                const label = cityName(p);
                chooseCity({ name, label: label && label !== name ? label : undefined, lat: p.lat, lng: p.lng, country_code: p.country_code ?? undefined });
              }}
              placeholder={tr(lang, "Усі міста", "All cities")}
              ariaLabel={tr(lang, "Місто", "City")}
              popEnd
              onEnter={commitCity}
              extra={cities.map(([name, { n, e }]) => ({
                key: name,
                primary: name,
                secondary: tp(lang, n, { one: "{n} подія", few: "{n} події", many: "{n} подій" }, { one: "{n} event", other: "{n} events" }),
                onSelect: () => chooseCity(pointOf(name, e)),
              }))}
            />
          </div>
        </div>
        <div className="row gap wrap near-row">
          <button type="button" className="btn quiet sm" onClick={nearMe} disabled={near === "locating"} aria-busy={near === "locating"}>
            <LocateFixed className="ico sm" />{near === "locating" ? tr(lang, "Визначаємо місце…", "Finding you…") : tr(lang, "Поруч зі мною", "Near me")}
          </button>
          {city && (
            <button type="button" className="btn quiet sm" onClick={() => chooseCity(null)}>
              <X className="ico sm" />{tr(lang, "Усі міста", "All cities")}
            </button>
          )}
          <span className="msg" role="status">
            {near === "denied" ? tr(lang, "Доступ до геопозиції заборонено — оберіть місто вручну.", "Location access is blocked — pick a city instead.")
              : near === "timeout" ? tr(lang, "Не вдалося визначити місце вчасно — спробуйте ще раз або оберіть місто.", "Finding your location took too long — try again or pick a city.")
              : near === "unavailable" ? tr(lang, "Не вдалося визначити місце — оберіть місто вручну.", "Couldn't find your location — pick a city instead.")
              : near === "unsupported" ? tr(lang, "Цей браузер не визначає місце — оберіть місто вручну.", "This browser can't share a location — pick a city instead.")
              : near === "unnamed" ? tr(lang, "Не вдалося назвати місто за геопозицією — оберіть його вручну.", "Couldn't name the city at your location — pick it instead.")
              : null}
          </span>
        </div>
        <div className="filter-row">
          <div className="chips scroll" role="group" aria-label={tr(lang, "Коли", "When")}>
            {WHEN.map(([k, label]) => (
              <button key={k} className={`chip ${when === k ? "active" : ""}`} aria-pressed={when === k} onClick={() => setWhen(k)}>{label}</button>
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
          <div className="section-head"><h2>{serverText(lang, sec.title)}</h2></div>
          <div className="hscroll">{sec.events.map(card)}</div>
        </section>
      ))}

      <section className="section">
        <div className="section-head">
          <h2>{filtering ? tr(lang, "Знайдено", "Results") : city ? tf(lang, "Усі події · {city}", "All events · {city}", { city: city.label ?? city.name }) : tr(lang, "Усі події", "All events")}</h2>
          {events && <span className="muted small">{shown.length}</span>}
        </div>
        {events === null ? (
          <div className="grid">{Array.from({ length: 8 }, (_, i) => <EventCardSkeleton key={i} />)}</div>
        ) : city && !cityHasEvents ? (
          <div className="empty">
            <div className="ei"><MapPin className="ico lg" /></div>
            <h3>{tf(lang, "У місті {city} ще немає подій", "No events in {city} yet", { city: city.label ?? city.name })}</h3>
            <p>{tr(lang, "Створіть першу — її побачать усі, хто шукає це місто.", "Create the first one — everyone looking at this city will see it.")}</p>
            <div className="row gap" style={{ justifyContent: "center" }}>
              <button className="btn ghost" onClick={() => chooseCity(null)}>{tr(lang, "Усі міста", "All cities")}</button>
              <Link className="btn" href={createHereHref(city)}><Plus className="ico sm" />{tr(lang, "Створити першу подію", "Create the first event")}</Link>
            </div>
          </div>
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
              {(filtering || city) && <button className="btn ghost" onClick={() => { setQ(""); setCat(""); setWhen("all"); chooseCity(null); }}>{tr(lang, "Скинути фільтри", "Reset filters")}</button>}
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
