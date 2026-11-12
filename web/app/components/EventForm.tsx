"use client";

import { useState, type FormEvent, type ReactNode } from "react";
import CityInput from "./CityInput";
import EventCard from "./EventCard";
import { AGE_RATINGS, EVENT_CATEGORIES, uploadImage, errorMessage, toIso, type EventInput, type EventDto } from "../../lib/api";
import { FREE_ONLY } from "../../lib/flags";
import { useLang, tr } from "../../lib/i18n";
import { categoryLabel } from "../../lib/ui";
import { ImageIcon, X, ChevronDown } from "./Icon";

export interface EventFormInitial {
  title: string;
  description: string;
  category: string;
  city: string;
  venue_address: string;
  starts_at_local: string;
  price: string;
  tickets_total: string;
  is_free: boolean;
  poster_url: string | null;
  video_url: string;
  links: string;
  is_draft: boolean;
  age_rating: string;
}

export const EMPTY_EVENT: EventFormInitial = {
  title: "", description: "", category: "Concert", city: "Київ", venue_address: "",
  starts_at_local: "", price: "", tickets_total: "50", is_free: true, poster_url: null, video_url: "", links: "",
  is_draft: false, age_rating: "",
};

/** ISO instant → the value a datetime-local input expects, in the visitor's own time zone. */
export function isoToLocalInput(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
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
}: {
  initial: EventFormInitial;
  submitLabel: string;
  onSubmit: (raw: EventFormInitial) => Promise<void>;
  allowDraft?: boolean;
  /** Places already booked: capacity cannot go below this. */
  minSeats?: number;
  /** Editing a published event: capacity is fixed after creation (the API does not change it). */
  editing?: boolean;
}) {
  const { lang } = useLang();
  const [f, setF] = useState<EventFormInitial>(FREE_ONLY ? { ...initial, is_free: true } : initial);
  const [busy, setBusy] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [touched, setTouched] = useState(false);
  const [more, setMore] = useState(!!(initial.video_url || initial.links));

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
  const startIso = toIso(f.starts_at_local);
  const problems = {
    title: !f.title.trim() ? tr(lang, "Вкажіть назву", "Enter a title") : null,
    date: !startIso ? tr(lang, "Вкажіть дату й час", "Pick a date and time") : new Date(startIso).getTime() < Date.now() ? tr(lang, "Дата вже минула", "That date has passed") : null,
    city: !f.city.trim() ? tr(lang, "Вкажіть місто", "Enter a city") : null,
    seats: editing ? null : !Number.isInteger(seats) || seats < Math.max(1, minSeats) || seats > 100000
      ? minSeats > 1 ? tr(lang, `Не менше ${minSeats} — стільки вже заброньовано`, `At least ${minSeats} — already booked`) : tr(lang, "Від 1 до 100 000 місць", "1 to 100,000 places")
      : null,
  };
  const invalid = Object.values(problems).some(Boolean);

  async function submit(e: FormEvent, asDraft = false) {
    e.preventDefault();
    setTouched(true);
    setError(null);
    if (!f.title.trim()) return setError(problems.title);
    if (invalid && !asDraft) return setError(tr(lang, "Заповніть позначені поля", "Fill in the highlighted fields"));
    setBusy(true);
    try {
      await onSubmit({ ...f, is_draft: asDraft });
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
    age_rating: f.age_rating || null,
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
            <div className="field">
              <label htmlFor="ev-date">{tr(lang, "Початок", "Starts")}</label>
              <input id="ev-date" className="input" type="datetime-local" value={f.starts_at_local} onChange={(e) => set("starts_at_local", e.target.value)} aria-invalid={!!show("date")} />
              {show("date") && <span className="err">{show("date")}</span>}
            </div>
            <div className="grid-2">
              <div className="field">
                <label htmlFor="ev-city">{tr(lang, "Місто", "City")}</label>
                <CityInput id="ev-city" value={f.city} onChange={(v) => set("city", v)} />
                {show("city") && <span className="err">{show("city")}</span>}
              </div>
              <div className="field">
                <label htmlFor="ev-addr">{tr(lang, "Адреса або місце", "Address or venue")}</label>
                <input id="ev-addr" className="input" value={f.venue_address} maxLength={200} onChange={(e) => set("venue_address", e.target.value)} placeholder={tr(lang, "Бар «Пульт», вул. Нижній Вал 21", "Venue, street and number")} />
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
        {allowDraft && <button type="button" className="btn ghost lg" disabled={busy || uploading} onClick={(e) => submit(e, true)}>{tr(lang, "Зберегти чернетку", "Save draft")}</button>}
        <button className="btn lg primary-glow" disabled={busy || uploading}>{busy ? tr(lang, "Зберігаємо…", "Saving…") : submitLabel}</button>
      </div>
    </form>
  );
}

export function toEventInput(f: EventFormInitial, startsAtIso: string | null): EventInput {
  const free = FREE_ONLY || f.is_free;
  return {
    title: f.title.trim(),
    description: f.description.trim() || null,
    category: f.category,
    city: f.city.trim() || null,
    venue_address: f.venue_address.trim() || null,
    starts_at: startsAtIso,
    price: free ? "0" : f.price.trim() || "0",
    tickets_total: Number(f.tickets_total) || 0,
    cost_type: free ? "free" : "paid",
    poster_url: f.poster_url,
    video_url: f.video_url.trim() || null,
    links: f.links.split(",").map((s) => s.trim()).filter(Boolean),
    age_rating: f.age_rating || null,
  };
}
