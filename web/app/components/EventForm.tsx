"use client";

import { useState, type FormEvent } from "react";
import CityInput from "./CityInput";
import { AGE_RATINGS, EVENT_CATEGORIES, uploadImage, errorMessage, type EventInput } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";

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
  title: "", description: "", category: EVENT_CATEGORIES[0], city: "Київ", venue_address: "",
  starts_at_local: "", price: "", tickets_total: "50", is_free: false, poster_url: null, video_url: "", links: "",
  is_draft: false, age_rating: "",
};

export default function EventForm({
  initial,
  submitLabel,
  onSubmit,
  allowDraft = false,
}: {
  initial: EventFormInitial;
  submitLabel: string;
  onSubmit: (raw: EventFormInitial) => Promise<void>;
  allowDraft?: boolean;
}) {
  const { lang } = useLang();
  const [f, setF] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function set<K extends keyof EventFormInitial>(key: K, value: EventFormInitial[K]) {
    setF((prev) => ({ ...prev, [key]: value }));
  }

  async function pickPoster(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    setUploading(true);
    setError(null);
    try {
      set("poster_url", await uploadImage(file));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setUploading(false);
    }
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    setError(null);
    if (!f.title.trim()) return setError(tr(lang, "Вкажіть назву", "Please enter a title"));
    setBusy(true);
    try {
      await onSubmit(f);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="form" onSubmit={submit}>
      <label className="lbl">{tr(lang, "Назва", "Title")}<input className="input" value={f.title} onChange={(e) => set("title", e.target.value)} /></label>
      <label className="lbl">{tr(lang, "Опис", "Description")}<textarea className="input" rows={3} value={f.description} onChange={(e) => set("description", e.target.value)} /></label>
      <label className="lbl">
        {tr(lang, "Категорія", "Category")}
        <select className="input" value={f.category} onChange={(e) => set("category", e.target.value)}>
          {EVENT_CATEGORIES.map((c) => <option key={c} value={c}>{c}</option>)}
        </select>
      </label>
      <div className="row gap">
        <label className="lbl" style={{ flex: 1 }}>{tr(lang, "Місто", "City")}<CityInput value={f.city} onChange={(v) => set("city", v)} /></label>
        <label className="lbl" style={{ flex: 2 }}>{tr(lang, "Адреса (для Google Maps)", "Address (for Google Maps)")}<input className="input" value={f.venue_address} onChange={(e) => set("venue_address", e.target.value)} /></label>
      </div>
      <label className="lbl">{tr(lang, "Дата і час", "Date and time")}<input className="input" type="datetime-local" value={f.starts_at_local} onChange={(e) => set("starts_at_local", e.target.value)} /></label>
      <label className="lbl row gap" style={{ flexDirection: "row", alignItems: "center" }}>
        <input type="checkbox" checked={f.is_free} onChange={(e) => set("is_free", e.target.checked)} /> {tr(lang, "Безкоштовна подія", "Free event")}
      </label>
      {!f.is_free && <label className="lbl">{tr(lang, "Ціна", "Price")}<input className="input" inputMode="decimal" value={f.price} onChange={(e) => set("price", e.target.value)} /></label>}
      <label className="lbl">{tr(lang, "Кількість квитків", "Number of tickets")}<input className="input" inputMode="numeric" value={f.tickets_total} onChange={(e) => set("tickets_total", e.target.value)} /></label>
      <label className="lbl">
        {tr(lang, "Вікове обмеження", "Age restriction")}
        <select className="input" value={f.age_rating} onChange={(e) => set("age_rating", e.target.value)}>
          {AGE_RATINGS.map((a) => (
            <option key={a} value={a}>{a === "" ? tr(lang, "Без обмежень", "No restriction") : `${a}+`}</option>
          ))}
        </select>
      </label>
      <label className="lbl">{tr(lang, "Постер-афіша", "Poster")}<input className="input" type="file" accept="image/*" onChange={pickPoster} /></label>
      {f.poster_url && <img src={f.poster_url} alt={tr(lang, "постер", "poster")} style={{ maxHeight: 150, borderRadius: 12 }} />}
      {uploading && <p className="muted">{tr(lang, "Завантаження зображення…", "Uploading image…")}</p>}
      <label className="lbl">{tr(lang, "Відео (YouTube URL)", "Video (YouTube URL)")}<input className="input" value={f.video_url} onChange={(e) => set("video_url", e.target.value)} /></label>
      <label className="lbl">{tr(lang, "Соцмережі (через кому)", "Social links (comma-separated)")}<input className="input" value={f.links} onChange={(e) => set("links", e.target.value)} /></label>
      {allowDraft && (
        <label className="lbl row gap" style={{ flexDirection: "row", alignItems: "center" }}>
          <input type="checkbox" checked={f.is_draft} onChange={(e) => set("is_draft", e.target.checked)} />
          {tr(lang, "Зберегти як чернетку (не публікувати)", "Save as draft (don't publish)")}
        </label>
      )}
      {error && <div className="error">{error}</div>}
      <button className="btn" disabled={busy || uploading}>{busy ? tr(lang, "Збереження…", "Saving…") : submitLabel}</button>
    </form>
  );
}

export function toEventInput(f: EventFormInitial, startsAtIso: string | null): EventInput {
  return {
    title: f.title.trim(),
    description: f.description.trim() || null,
    category: f.category,
    city: f.city.trim() || null,
    venue_address: f.venue_address.trim() || null,
    starts_at: startsAtIso,
    price: f.is_free ? "0" : f.price.trim() || "0",
    tickets_total: Number(f.tickets_total) || 0,
    cost_type: f.is_free ? "free" : "paid",
    poster_url: f.poster_url,
    video_url: f.video_url.trim() || null,
    links: f.links.split(",").map((s) => s.trim()).filter(Boolean),
    age_rating: f.age_rating || null,
  };
}
