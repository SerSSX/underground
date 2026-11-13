"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import EventForm, { EMPTY_EVENT, toEventInput, type EventFormInitial } from "../../components/EventForm";
import { createEvent, getToken, toIso, browserTz, type EventDto } from "../../../lib/api";
import { useLang, tr, tf } from "../../../lib/i18n";
import { hasPoint } from "../../../lib/geo-core.mjs";
import { Toast, useToast } from "../../components/Toast";
import { CircleCheck, Copy, ExternalLink, Users } from "../../components/Icon";

export default function NewEventPage() {
  const router = useRouter();
  const { lang } = useLang();
  const [created, setCreated] = useState<EventDto | null>(null);
  const [toast, showToast] = useToast();
  const [tz, setTz] = useState<string | null>(null);
  // "Create the first event in {city}" from the home page arrives as ?city=; read once on the client.
  const [initial, setInitial] = useState<EventFormInitial | null>(null);
  const [hint, setHint] = useState<{ lat: number; lng: number; cc: string | null } | null>(null);
  useEffect(() => {
    setTz(browserTz());
    const q = new URLSearchParams(window.location.search);
    const city = (q.get("city") ?? "").replace(/\s+/g, " ").trim().slice(0, 100);
    // The city's point and country from the home page, for its time zone; checked like any URL input.
    const lat = Number(q.get("lat"));
    const lng = Number(q.get("lng"));
    const cc = (q.get("cc") ?? "").trim();
    const point = city && q.get("lat") && q.get("lng") && hasPoint({ lat, lng }) ? { lat, lng } : null;
    const code = /^[A-Za-z]{2}$/.test(cc) ? cc.toUpperCase() : null;
    setHint(point ? { ...point, cc: code } : null);
    setInitial({ ...EMPTY_EVENT, city, country_code: city ? code : null });
  }, []);

  useEffect(() => {
    if (!getToken()) router.replace("/login?next=/organizer/new");
  }, [router]);

  if (created) {
    const link = `${window.location.origin}/events/${created.id}`;
    const msg = tf(lang, "Приходь на «{title}»", "Come to “{title}”", { title: created.title });
    const draft = created.status === "draft";
    return (
      <main className="container tight">
        <div className="card" style={{ marginTop: 24, padding: 28, textAlign: "center", borderRadius: "var(--r-xl)" }}>
          <CircleCheck style={{ width: 56, height: 56, color: "var(--ok-fg)", margin: "0 auto 12px" }} />
          <h1 style={{ fontSize: 28 }}>{draft ? tr(lang, "Чернетку збережено", "Draft saved") : tr(lang, "Подію опубліковано!", "Your event is live!")}</h1>
          <p className="muted" style={{ marginTop: 8 }}>
            {draft ? tr(lang, "Її бачите лише ви. Опублікуйте, коли все буде готово.", "Only you can see it. Publish when ready.") : tr(lang, "Вона вже в афіші. Поділіться посиланням — так прийде більше людей.", "It's in the listings now. Share the link to bring more people.")}
          </p>
          {!draft && (
            <>
              <div className="row gap" style={{ marginTop: 20 }}>
                <input className="input grow" readOnly value={link} onFocus={(e) => e.currentTarget.select()} aria-label={tr(lang, "Посилання на подію", "Event link")} />
                <button type="button" className="btn icon" onClick={() => void navigator.clipboard?.writeText(link).then(() => showToast(tr(lang, "Скопійовано", "Copied")))} aria-label={tr(lang, "Скопіювати", "Copy")}><Copy className="ico sm" /></button>
              </div>
              <div className="row gap" style={{ marginTop: 10, justifyContent: "center" }}>
                <a className="btn secondary sm" target="_blank" rel="noreferrer" href={`https://t.me/share/url?url=${encodeURIComponent(link)}&text=${encodeURIComponent(msg)}`}>Telegram</a>
                <a className="btn secondary sm" target="_blank" rel="noreferrer" href={`https://wa.me/?text=${encodeURIComponent(`${msg} ${link}`)}`}>WhatsApp</a>
                <a className="btn secondary sm" target="_blank" rel="noreferrer" href={`viber://forward?text=${encodeURIComponent(`${msg} ${link}`)}`}>Viber</a>
              </div>
            </>
          )}
          <div className="row gap" style={{ marginTop: 22, justifyContent: "center" }}>
            {!draft && <Link className="btn" href={`/events/${created.id}`}><ExternalLink className="ico sm" />{tr(lang, "Відкрити подію", "Open event")}</Link>}
            <Link className="btn ghost" href={`/organizer/${created.id}`}><Users className="ico sm" />{tr(lang, "Керувати", "Manage")}</Link>
          </div>
        </div>
        <Toast msg={toast} />
      </main>
    );
  }

  return (
    <main className="container">
      <div className="page-head">
        <div>
          <h1>{tr(lang, "Нова подія", "New event")}</h1>
          <p className="muted">{tr(lang, "Кілька хвилин — і подія в афіші. Вхід для гостей безкоштовний.", "A few minutes and it's live. Entry is free for guests.")}</p>
        </div>
      </div>
      {!initial ? <div className="skeleton" style={{ height: 320 }} /> : <EventForm
        initial={initial}
        cityHint={hint}
        tz={tz}
        submitLabel={tr(lang, "Опублікувати", "Publish")}
        allowDraft
        onSubmit={async (f) => {
          const zone = f.timezone ?? tz;
          const ev = await createEvent({ ...toEventInput(f, toIso(f.starts_at_local, zone), zone), timezone: zone, status: f.is_draft ? "draft" : "published" });
          setCreated(ev);
          window.scrollTo({ top: 0 });
        }}
      />}
    </main>
  );
}
