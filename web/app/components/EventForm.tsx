"use client";

import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import CityInput from "./CityInput";
import AddressInput from "./AddressInput";
import EventCard from "./EventCard";
import { AGE_RATINGS, EVENT_CATEGORIES, uploadImage, errorMessage, toIso, tzOffsetMs, searchCities, type EventInput, type EventDto } from "../../lib/api";
import { FREE_ONLY } from "../../lib/flags";
import { useLang, tr, tf } from "../../lib/i18n";
import { categoryLabel, countryTz, tzLabel } from "../../lib/ui";
import { isZone, zoneFor } from "../../lib/geo";
import { addressLabel, cityStoredName, trustedCityRow, CITY_RADIUS_KM, hasPoint, haversineKm, type GeoPlace, type Point } from "../../lib/geo-core.mjs";
import { ImageIcon, X, ChevronDown } from "./Icon";

export interface EventFormInitial {
  title: string;
  description: string;
  category: string;
  city: string;
  venue_address: string;
  starts_at_local: string;
  ends_at_local: string;
  price: string;
  tickets_total: string;
  is_free: boolean;
  poster_url: string | null;
  video_url: string;
  links: string;
  is_draft: boolean;
  age_rating: string;
  /** IANA zone of the event's place; undefined = not known, the `tz` prop applies. */
  timezone?: string | null;
  /** The picked address's point and country (TASK-303 G2); null when the address was typed. */
  lat?: number | null;
  lng?: number | null;
  country?: string | null;
  country_code?: string | null;
}

export const EMPTY_EVENT: EventFormInitial = {
  title: "", description: "", category: "Concert", city: "", venue_address: "",
  starts_at_local: "", ends_at_local: "", price: "", tickets_total: "50", is_free: true, poster_url: null, video_url: "", links: "",
  is_draft: false, age_rating: "",
};

/** ISO instant → the value a datetime-local input expects: wall-clock time in the event's zone (or the
 *  visitor's, when none). */
export function isoToLocalInput(iso: string | null | undefined, tz?: string | null): string {
  if (!iso) return "";
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return "";
  const d = new Date(tz ? t + tzOffsetMs(t, tz) : t - new Date(t).getTimezoneOffset() * 60_000);
  return d.toISOString().slice(0, 16);
}

function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <section className="card" style={{ padding: 22 }}>
      <h2 style={{ fontSize: 17 }}>{title}</h2>
      {hint && <p className="muted small" style={{ marginTop: 4 }}>{hint}</p>}
      <div className="form" style={{ marginTop: 16 }}>{children}</div>
    </section>
  );
}

export default function EventForm({
  initial,
  submitLabel,
  onSubmit,
  allowDraft = false,
  minSeats = 1,
  editing = false,
  tz = null,
  cityHint = null,
}: {
  initial: EventFormInitial;
  submitLabel: string;
  onSubmit: (raw: EventFormInitial) => Promise<void>;
  allowDraft?: boolean;
  /** Places already booked: capacity cannot go below this. */
  minSeats?: number;
  /** Editing a published event: capacity is fixed after creation (the API does not change it). */
  editing?: boolean;
  /** Zone the date input is in: the event's when editing, the organiser's own when creating. */
  tz?: string | null;
  /** The prefilled city's point and country (home → "Create the first event"): sets the zone and the address
   *  bias, never the event's own pin (fix F7). */
  cityHint?: (Point & { cc?: string | null }) | null;
}) {
  const { lang } = useLang();
  const [f, setF] = useState<EventFormInitial>(FREE_ONLY ? { ...initial, is_free: true } : initial);
  const [busy, setBusy] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [touched, setTouched] = useState(false);
  const [more, setMore] = useState(!!(initial.video_url || initial.links));
  // The event runs on its place's clock (TASK-303 G2). A picked point asks the API for the zone an event there
  // gets (GET /api/geo/timezone, the same rule as create); a typed city falls back to our gazetteer by name;
  // nothing known → the `tz` prop (the browser's zone when creating, the stored zone when editing). The one
  // `zone` below converts the times AND is sent as the event's zone, so the stored instant and zone agree.
  const [cityPoint, setCityPoint] = useState<Point | null>(
    hasPoint(initial) ? { lat: initial.lat as number, lng: initial.lng as number } : cityHint ? { lat: cityHint.lat, lng: cityHint.lng } : null,
  );
  // A new object per keystroke, so the lookup below always re-runs (and always clears zonePending).
  const [typedCity, setTypedCity] = useState<{ text: string } | null>(!editing && !cityHint && initial.city.trim() ? { text: initial.city } : null);
  const [zonePending, setZonePending] = useState(!editing && !!initial.city.trim());
  const zoneSeq = useRef(0);

  async function settleZone(place: { point?: Point | null; cc?: string | null; city: string }) {
    const seq = ++zoneSeq.current;
    setZonePending(true);
    let zone: string | null = null;
    let center: Point | null = null;
    let rowCc: string | null = null;
    // A hung request must not keep the submit button disabled: give up after 6 s and use the fallback.
    const ctl = new AbortController();
    const timer = setTimeout(() => ctl.abort(), 6000);
    if (place.point) {
      try { zone = await zoneFor(place.point.lat, place.point.lng, place.cc, ctl.signal); } catch { /* the city name below */ }
    }
    const city = place.city.trim();
    if (!zone && city.length >= 2) {
      try {
        // Only a row the API itself would use for this event: seeded, or learned in the country the form
        // knows (G1 fix batch: learned rows answer only their own country). Else the fallback zone.
        const hit = trustedCityRow(await searchCities(city, ctl.signal), city, place.cc);
        if (hit) {
          zone = (hit.timezone && isZone(hit.timezone) ? hit.timezone : null) ?? countryTz(hit.country);
          center = { lat: hit.lat, lng: hit.lng };
          rowCc = hit.country_code && /^[A-Za-z]{2}$/.test(hit.country_code) ? hit.country_code.toUpperCase() : null;
        }
      } catch { /* lookup failed: the fallback zone */ }
    }
    clearTimeout(timer);
    if (seq !== zoneSeq.current) return; // a newer place won
    setF((prev) => ({ ...prev, timezone: zone ?? undefined }));
    if (center && !place.point) setCityPoint(center);
    // A typed city the gazetteer knows: send its country code with the event (the form had none).
    if (rowCc && !place.point) setF((prev) => (prev.country_code ? prev : { ...prev, country_code: rowCc }));
    setZonePending(false);
  }

  // A typed city: look it up once the typing pauses.
  useEffect(() => {
    if (typedCity === null) return;
    // cc: what the form already knows (e.g. ?cc= from the home link); a typed city clears it otherwise.
    const t = setTimeout(() => void settleZone({ city: typedCity.text, cc: f.country_code }), 400);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [typedCity]);

  // A prefilled city with its point: its zone straight from the API, as for a picked city.
  useEffect(() => {
    if (!editing && cityHint && initial.city.trim()) void settleZone({ point: cityHint, cc: cityHint.cc, city: initial.city });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Editing: a stored point that is only the city's gazetteer centre (the address was typed, not picked) is
  // not a pin — the hint must not say "pinned", and the Maps link already treats it as no pin (fix F8).
  useEffect(() => {
    const city = initial.city.trim();
    if (!editing || !hasPoint(initial) || city.length < 2) return;
    const ctl = new AbortController();
    searchCities(city, ctl.signal)
      .then((list) => {
        // Several rows may share a name (one per country): the stored point is a centre if it equals any of them.
        const centre = list.some((x) => x.name.toLowerCase() === city.toLowerCase()
          && Math.abs(x.lat - (initial.lat as number)) <= 1e-6 && Math.abs(x.lng - (initial.lng as number)) <= 1e-6);
        if (!centre) return;
        // Untouched since load: drop the centre (the API puts it back from the city name on save).
        setF((prev) => (prev.lat === initial.lat && prev.lng === initial.lng ? { ...prev, lat: null, lng: null } : prev));
      })
      .catch(() => { /* keep what is stored */ });
    return () => ctl.abort();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Forget the previous place's zone at once: until the new one is known, the fallback applies. */
  function placeChanging() {
    zoneSeq.current++;
    setZonePending(true);
    setF((prev) => ({ ...prev, timezone: undefined }));
  }

  function typeCity(text: string) {
    // Only spacing or letter case changed ("київ " → "Київ"): the same city, so pin, country and zone stay (F8).
    const norm = (v: string) => v.replace(/\s+/g, " ").trim().toLowerCase();
    if (norm(text) !== "" && norm(text) === norm(f.city)) {
      setF((prev) => ({ ...prev, city: text }));
      return;
    }
    placeChanging();
    // The picked point and country described the previous place.
    setF((prev) => ({ ...prev, city: text, lat: null, lng: null, country: null, country_code: null }));
    setCityPoint(null);
    setTypedCity({ text });
  }

  function pickCity(p: GeoPlace, name: string) {
    placeChanging();
    setTypedCity(null);
    // An address picked earlier stays only if it lies in this city.
    setF((prev) => {
      const keep = hasPoint(prev) && haversineKm(prev.lat as number, prev.lng as number, p.lat, p.lng) <= CITY_RADIUS_KM;
      return { ...prev, city: name, country: p.country, country_code: p.country_code, ...(keep ? {} : { lat: null, lng: null }) };
    });
    setCityPoint({ lat: p.lat, lng: p.lng });
    void settleZone({ point: { lat: p.lat, lng: p.lng }, cc: p.country_code, city: name });
  }

  function typeAddress(text: string) {
    // The pin belonged to the picked suggestion; edited text may name another place. City and zone stay.
    setF((prev) => ({ ...prev, venue_address: text, lat: null, lng: null }));
  }

  function pickAddress(p: GeoPlace) {
    placeChanging();
    setTypedCity(null);
    const city = cityStoredName(p) || f.city;
    // A whole city picked in the address field names the city, not a spot: no pin (fix F8).
    const pin = p.kind !== "city";
    setF((prev) => ({
      ...prev, venue_address: pin ? addressLabel(p) : prev.venue_address, city,
      lat: pin ? p.lat : null, lng: pin ? p.lng : null,
      country: p.country ?? prev.country ?? null, country_code: p.country_code ?? prev.country_code ?? null,
    }));
    setCityPoint({ lat: p.lat, lng: p.lng });
    void settleZone({ point: { lat: p.lat, lng: p.lng }, cc: p.country_code, city });
  }

  const zone = f.timezone ?? tz ?? null;

  function set<K extends keyof EventFormInitial>(key: K, value: EventFormInitial[K]) {
    setF((prev) => ({ ...prev, [key]: value }));
  }

  async function pickPoster(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    if (file.size > 5 * 1024 * 1024) return setError(tr(lang, "Зображення до 5 МБ", "Image must be under 5 MB"));
    setUploading(true);
    setError(null);
    try {
      set("poster_url", await uploadImage(file));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setUploading(false);
      e.target.value = "";
    }
  }

  const seats = Number(f.tickets_total);
  const startIso = toIso(f.starts_at_local, zone);
  const endIso = toIso(f.ends_at_local, zone);
  const problems = {
    title: !f.title.trim() ? tr(lang, "Вкажіть назву", "Enter a title") : null,
    date: !startIso ? tr(lang, "Вкажіть дату й час", "Pick a date and time") : new Date(startIso).getTime() < Date.now() ? tr(lang, "Дата вже минула", "That date has passed") : null,
    end: endIso && startIso && Date.parse(endIso) <= Date.parse(startIso) ? tr(lang, "Кінець має бути після початку", "The end must be after the start") : null,
    city: !f.city.trim() ? tr(lang, "Вкажіть місто", "Enter a city") : null,
    seats: editing ? null : !Number.isInteger(seats) || seats < Math.max(1, minSeats) || seats > 100000
      ? minSeats > 1 ? tf(lang, "Не менше {n} — стільки вже заброньовано", "At least {n} — already booked", { n: minSeats }) : tr(lang, "Від 1 до 100 000 місць", "1 to 100,000 places")
      : null,
  };
  const invalid = Object.values(problems).some(Boolean);

  async function submit(e: FormEvent, asDraft = false) {
    e.preventDefault();
    setTouched(true);
    setError(null);
    if (!f.title.trim()) return setError(problems.title);
    if (invalid && !asDraft) return setError(tr(lang, "Заповніть позначені поля", "Fill in the highlighted fields"));
    if (zonePending) return;
    setBusy(true);
    try {
      await onSubmit({ ...f, is_draft: asDraft, timezone: zone });
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  }

  const show = (k: keyof typeof problems) => (touched ? problems[k] : null);
  const preview: EventDto = {
    id: "preview", organizer_id: null, title: f.title || tr(lang, "Назва події", "Event title"), description: f.description,
    poster_url: f.poster_url, category: f.category, city: f.city || null, venue_address: f.venue_address || null,
    starts_at: startIso, price: f.price || "0", currency: "UAH", tickets_left: seats > 0 ? seats : 0, tickets_total: seats,
    status: "published", cost_type: f.is_free ? "free" : "paid", video_url: null, gallery_urls: [], links: [], sales_closed: false,
    age_rating: f.age_rating || null, timezone: zone ?? undefined,
  };

  return (
    <form onSubmit={(e) => submit(e)} noValidate>
      <div className="form-layout">
        <div className="stack lg">
          <Section title={tr(lang, "Основне", "Basics")}>
            <div className="field">
              <label htmlFor="ev-title">{tr(lang, "Назва", "Title")}</label>
              <input id="ev-title" className="input" value={f.title} maxLength={140} onChange={(e) => set("title", e.target.value)} aria-invalid={!!show("title")} placeholder={tr(lang, "Наприклад: Джем-сейшен у підвалі", "e.g. Basement jam session")} />
              {show("title") && <span className="err">{show("title")}</span>}
            </div>
            <div className="field">
              <label htmlFor="ev-cat">{tr(lang, "Категорія", "Category")}</label>
              <select id="ev-cat" className="input" value={f.category} onChange={(e) => set("category", e.target.value)}>
                {!EVENT_CATEGORIES.some((c) => c.toLowerCase() === f.category.toLowerCase()) && <option value={f.category}>{categoryLabel(f.category, lang)}</option>}
                {[...EVENT_CATEGORIES].sort((a, b) => categoryLabel(a, lang).localeCompare(categoryLabel(b, lang))).map((c) => (
                  <option key={c} value={c.toLowerCase() === f.category.toLowerCase() ? f.category : c}>{categoryLabel(c, lang)}</option>
                ))}
              </select>
            </div>
            <div className="field">
              <label htmlFor="ev-desc">{tr(lang, "Опис", "Description")}</label>
              <textarea id="ev-desc" className="input" rows={5} value={f.description} maxLength={5000} onChange={(e) => set("description", e.target.value)} placeholder={tr(lang, "Що буде, для кого, що взяти з собою", "What happens, who it's for, what to bring")} />
            </div>
          </Section>

          <Section title={tr(lang, "Коли й де", "When and where")}>
            <div className="grid-2">
            <div className="field">
              <label htmlFor="ev-date">{tr(lang, "Початок", "Starts")}{zone ? <span className="subtle" style={{ fontWeight: 500 }} title={zone}> · {tzLabel(zone, lang)}</span> : null}</label>
              <input id="ev-date" className="input" type="datetime-local" value={f.starts_at_local} onChange={(e) => set("starts_at_local", e.target.value)} aria-invalid={!!show("date")} />
              {show("date") ? <span className="err">{show("date")}</span>
                : !editing && f.timezone === undefined && !zonePending && zone ? (
                  // The place's zone is not known: times are in the organiser's own (fix F7). Say so.
                  <span className="hint">{tf(lang, "Час за вашим поясом ({zone}): оберіть місто чи адресу з підказок, щоб узяти пояс місця.", "Times are in your own zone ({zone}): pick the city or address from the suggestions to use the venue's.", { zone: tzLabel(zone, lang) })}</span>
                ) : null}
            </div>
            <div className="field">
              <label htmlFor="ev-end">{tr(lang, "Кінець (необовʼязково)", "Ends (optional)")}</label>
              <input id="ev-end" className="input" type="datetime-local" value={f.ends_at_local} onChange={(e) => set("ends_at_local", e.target.value)} aria-invalid={!!show("end")} />
              {show("end") ? <span className="err">{show("end")}</span> : <span className="hint">{tr(lang, "До цього часу гості можуть пройти за QR.", "Guests can check in until then.")}</span>}
            </div>
            </div>
            <div className="grid-2">
              <div className="field">
                <label htmlFor="ev-city">{tr(lang, "Місто", "City")}</label>
                <CityInput id="ev-city" value={f.city} onChange={typeCity} onPick={pickCity} ariaInvalid={!!show("city")} />
                {show("city") && <span className="err">{show("city")}</span>}
              </div>
              <div className="field">
                <label htmlFor="ev-addr">{tr(lang, "Адреса або місце", "Address or venue")}</label>
                <AddressInput id="ev-addr" value={f.venue_address} onChange={typeAddress} onPick={pickAddress} bias={cityPoint} />
                <span className="hint">{hasPoint(f) ? tr(lang, "Місце позначено на мапі.", "The place is pinned on the map.") : tr(lang, "Оберіть підказку — і гості знайдуть місце на мапі.", "Pick a suggestion so guests find the place on the map.")}</span>
              </div>
            </div>
          </Section>

          <Section title={tr(lang, "Місця", "Places")} hint={FREE_ONLY ? tr(lang, "Вхід безкоштовний. Кожен гість може забронювати до 5 місць.", "Entry is free. Each guest can book up to 5 places.") : undefined}>
            <div className="grid-2">
              {editing ? (
                <div className="field">
                  <span className="label">{tr(lang, "Скільки місць", "Capacity")}</span>
                  <div className="input" style={{ display: "flex", alignItems: "center", color: "var(--muted)" }}>{f.tickets_total}</div>
                  <span className="hint">{tr(lang, "Кількість місць задається під час створення.", "Capacity is set when the event is created.")}</span>
                </div>
              ) : (
              <div className="field">
                <label htmlFor="ev-seats">{tr(lang, "Скільки місць", "Capacity")}</label>
                <input id="ev-seats" className="input" inputMode="numeric" value={f.tickets_total} onChange={(e) => set("tickets_total", e.target.value.replace(/\D/g, ""))} aria-invalid={!!show("seats")} />
                {show("seats") && <span className="err">{show("seats")}</span>}
              </div>
              )}
              <div className="field">
                <label htmlFor="ev-age">{tr(lang, "Вікове обмеження", "Age limit")}</label>
                <select id="ev-age" className="input" value={f.age_rating} onChange={(e) => set("age_rating", e.target.value)}>
                  {AGE_RATINGS.map((a) => <option key={a} value={a}>{a === "" ? tr(lang, "Без обмежень", "None") : `${a}+`}</option>)}
                </select>
              </div>
            </div>
            {!FREE_ONLY && (
              <>
                <label className="check"><input type="checkbox" checked={f.is_free} onChange={(e) => set("is_free", e.target.checked)} />{tr(lang, "Безкоштовна подія", "Free event")}</label>
                {!f.is_free && (
                  <div className="field"><label htmlFor="ev-price">{tr(lang, "Ціна", "Price")}</label><input id="ev-price" className="input" inputMode="decimal" value={f.price} onChange={(e) => set("price", e.target.value)} /></div>
                )}
              </>
            )}
          </Section>

          <Section title={tr(lang, "Постер", "Poster")} hint={tr(lang, "Горизонтальне фото 4:3 або квадрат, до 5 МБ. Без постера покажемо кольорову обкладинку з назвою.", "A 4:3 or square image, up to 5 MB. Without one we show a coloured cover with the title.")}>
            {f.poster_url ? (
              <div style={{ position: "relative", maxWidth: 360 }}>
                <img src={f.poster_url} alt={tr(lang, "Постер", "Poster")} style={{ width: "100%", borderRadius: 16, aspectRatio: "4 / 3", objectFit: "cover" }} />
                <button type="button" className="round-btn" style={{ position: "absolute", top: 10, right: 10 }} onClick={() => set("poster_url", null)} aria-label={tr(lang, "Прибрати постер", "Remove poster")}><X className="ico sm" /></button>
              </div>
            ) : (
              <label className="empty" style={{ cursor: "pointer", padding: 28 }}>
                <span className="ei"><ImageIcon className="ico lg" /></span>
                <span style={{ display: "block", fontWeight: 650 }}>{uploading ? tr(lang, "Завантажуємо…", "Uploading…") : tr(lang, "Додати зображення", "Add an image")}</span>
                <input type="file" accept="image/jpeg,image/png,image/webp" onChange={pickPoster} className="sr-only" />
              </label>
            )}
          </Section>

          <section className="card" style={{ padding: 0 }}>
            <button type="button" className="btn quiet block" style={{ justifyContent: "space-between", padding: "0 22px", minHeight: 56, borderRadius: "var(--r-lg)" }} onClick={() => setMore((m) => !m)} aria-expanded={more}>
              <span className="strong" style={{ color: "var(--text)" }}>{tr(lang, "Додатково: відео й посилання", "Extra: video and links")}</span>
              <ChevronDown className="ico" style={{ transform: more ? "rotate(180deg)" : undefined }} />
            </button>
            {more && (
              <div className="form" style={{ padding: "0 22px 22px" }}>
                <div className="field"><label htmlFor="ev-video">{tr(lang, "Відео (YouTube)", "Video (YouTube)")}</label><input id="ev-video" className="input" type="url" value={f.video_url} onChange={(e) => set("video_url", e.target.value)} placeholder="https://youtube.com/…" /></div>
                <div className="field"><label htmlFor="ev-links">{tr(lang, "Посилання через кому", "Links, comma-separated")}</label><input id="ev-links" className="input" value={f.links} onChange={(e) => set("links", e.target.value)} placeholder="https://instagram.com/…, https://t.me/…" /></div>
              </div>
            )}
          </section>
        </div>

        <aside className="hide-mobile form-preview" style={{ position: "sticky", top: "calc(var(--nav-h) + 20px)" }}>
          <div className="muted xs strong" style={{ textTransform: "uppercase", letterSpacing: ".06em", marginBottom: 10 }}>{tr(lang, "Так побачать гості", "How guests will see it")}</div>
          <div style={{ pointerEvents: "none" }}><EventCard event={preview} /></div>
        </aside>
      </div>

      {error && <div className="alert error" style={{ marginTop: 16 }}>{error}</div>}
      <div className="row gap wrap" style={{ marginTop: 20, justifyContent: "flex-end" }}>
        {allowDraft && <button type="button" className="btn ghost lg" disabled={busy || uploading || zonePending} onClick={(e) => submit(e, true)}>{tr(lang, "Зберегти чернетку", "Save draft")}</button>}
        <button className="btn lg primary-glow" disabled={busy || uploading || zonePending}>{busy ? tr(lang, "Зберігаємо…", "Saving…") : submitLabel}</button>
      </div>
    </form>
  );
}

export function toEventInput(f: EventFormInitial, startsAtIso: string | null, tz?: string | null): EventInput {
  const free = FREE_ONLY || f.is_free;
  return {
    title: f.title.trim(),
    description: f.description.trim() || null,
    category: f.category,
    city: f.city.trim() || null,
    venue_address: f.venue_address.trim() || null,
    starts_at: startsAtIso,
    ends_at: toIso(f.ends_at_local, tz) ?? null,
    price: free ? "0" : f.price.trim() || "0",
    tickets_total: Number(f.tickets_total) || 0,
    cost_type: free ? "free" : "paid",
    poster_url: f.poster_url,
    video_url: f.video_url.trim() || null,
    links: f.links.split(",").map((s) => s.trim()).filter(Boolean),
    age_rating: f.age_rating || null,
    // Both coordinates or neither (the API refuses half a point).
    lat: hasPoint(f) ? f.lat : null,
    lng: hasPoint(f) ? f.lng : null,
    country: f.country?.trim() || null,
    country_code: f.country_code && /^[A-Za-z]{2}$/.test(f.country_code) ? f.country_code.toUpperCase() : null,
  };
}
