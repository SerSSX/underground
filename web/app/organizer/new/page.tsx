"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import EventForm, { EMPTY_EVENT, toEventInput } from "../../components/EventForm";
import { createEvent, getToken, toIso, type EventDto } from "../../../lib/api";
import { useLang, tr } from "../../../lib/i18n";

export default function NewEventPage() {
  const router = useRouter();
  const { lang } = useLang();
  const [created, setCreated] = useState<EventDto | null>(null);

  useEffect(() => {
    if (!getToken()) router.push("/login?next=/organizer/new");
  }, [router]);

  // Post-publish viral hook: the moment the event is live, prompt to share its link.
  if (created) {
    const link = typeof window !== "undefined" ? `${window.location.origin}/events/${created.id}` : "";
    const msg = tr(lang, `Приходь на «${created.title}»`, `Come to “${created.title}”`);
    return (
      <main className="container narrow">
        <h1 style={{ fontSize: 24 }}>{tr(lang, "🎉 Опубліковано!", "🎉 Published!")}</h1>
        <p className="muted">{tr(lang, "Поділись лінком, щоб прийшло більше людей.", "Share the link so more people come.")}</p>
        <input className="input" readOnly value={link} onFocus={(e) => e.currentTarget.select()} style={{ marginBottom: 10 }} />
        <div className="row gap" style={{ marginBottom: 16 }}>
          <button
            type="button"
            className="btn ghost sm"
            onClick={() => void navigator.clipboard?.writeText(link)}
          >
            {tr(lang, "Скопіювати", "Copy")}
          </button>
          <a className="btn ghost sm" target="_blank" rel="noreferrer" href={`https://t.me/share/url?url=${encodeURIComponent(link)}&text=${encodeURIComponent(msg)}`}>Telegram</a>
          <a className="btn ghost sm" target="_blank" rel="noreferrer" href={`https://wa.me/?text=${encodeURIComponent(`${msg} ${link}`)}`}>WhatsApp</a>
        </div>
        <div className="row gap">
          <button type="button" className="btn" onClick={() => router.push(`/events/${created.id}`)}>{tr(lang, "Відкрити подію", "Open event")}</button>
          <button type="button" className="btn ghost" onClick={() => router.push("/organizer")}>{tr(lang, "Мої події", "My events")}</button>
        </div>
      </main>
    );
  }

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24 }}>{tr(lang, "Нова подія", "New event")}</h1>
      <EventForm
        initial={EMPTY_EVENT}
        submitLabel={tr(lang, "Опублікувати", "Publish")}
        allowDraft
        onSubmit={async (f) => {
          const ev = await createEvent({
            ...toEventInput(f, toIso(f.starts_at_local)),
            status: f.is_draft ? "draft" : "published",
          });
          setCreated(ev);
        }}
      />
    </main>
  );
}
