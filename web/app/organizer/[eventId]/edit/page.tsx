"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import EventForm, { toEventInput, isoToLocalInput, type EventFormInitial } from "../../../components/EventForm";
import { myEvents, updateEvent, getToken, toIso, errorMessage, type EventDto } from "../../../../lib/api";
import { useLang, tr } from "../../../../lib/i18n";
import { ChevronLeft } from "../../../components/Icon";

export default function EditEventPage() {
  const { eventId } = useParams<{ eventId: string }>();
  const router = useRouter();
  const { lang } = useLang();
  const [event, setEvent] = useState<EventDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!getToken()) {
      router.replace(`/login?next=${encodeURIComponent(`/organizer/${eventId}/edit`)}`);
      return;
    }
    myEvents()
      .then((list) => {
        const found = list.find((e) => e.id === eventId);
        if (!found) setError(tr(lang, "Цю подію може редагувати лише її організатор.", "Only the organiser can edit this event."));
        setEvent(found ?? null);
      })
      .catch((e) => setError(errorMessage(e)));
  }, [eventId, router, lang]);

  if (error && !event) return <main className="container narrow"><div className="alert error" style={{ marginTop: 24 }}>{error}</div></main>;
  if (!event) return <main className="container"><div className="skeleton" style={{ height: 320 }} /></main>;

  const initial: EventFormInitial = {
    title: event.title,
    description: event.description ?? "",
    category: event.category,
    city: event.city ?? "",
    venue_address: event.venue_address ?? "",
    starts_at_local: isoToLocalInput(event.starts_at, event.timezone),
    ends_at_local: isoToLocalInput(event.ends_at, event.timezone),
    price: event.price,
    tickets_total: String(event.tickets_total ?? event.tickets_left),
    is_free: event.cost_type === "free",
    poster_url: event.poster_url,
    video_url: event.video_url ?? "",
    links: event.links.join(", "),
    is_draft: event.status === "draft",
    age_rating: event.age_rating ?? "",
  };
  const draft = event.status === "draft";

  return (
    <main className="container">
      <Link href={`/organizer/${event.id}`} className="btn quiet sm" style={{ marginLeft: -12, marginBottom: 8 }}><ChevronLeft className="ico sm" />{event.title}</Link>
      <div className="page-head"><h1>{tr(lang, "Редагування події", "Edit event")}</h1></div>
      <EventForm
        initial={initial}
        editing
        tz={event.timezone ?? null}
        submitLabel={draft ? tr(lang, "Опублікувати", "Publish") : tr(lang, "Зберегти зміни", "Save changes")}
        allowDraft={draft}
        onSubmit={async (f) => {
          // tickets_total and cost_type are fixed after creation; send only what the API updates.
          const { tickets_total: _t, cost_type: _c, ...body } = toEventInput(f, toIso(f.starts_at_local, event.timezone), event.timezone);
          await updateEvent(eventId, { ...body, ...(draft ? { status: f.is_draft ? "draft" : "published" } : {}) });
          router.push(`/organizer/${eventId}`);
        }}
      />
    </main>
  );
}
