"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import EventForm, { toEventInput, type EventFormInitial } from "../../../components/EventForm";
import { myEvents, updateEvent, cancelEvent, getToken, toIso, errorMessage, type EventDto } from "../../../../lib/api";

export default function EditEventPage() {
  const { eventId } = useParams<{ eventId: string }>();
  const router = useRouter();
  const [event, setEvent] = useState<EventDto | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!getToken()) {
      router.push("/login");
      return;
    }
    myEvents()
      .then((list) => {
        const found = list.find((e) => e.id === eventId);
        if (!found) setError("Подію не знайдено серед ваших");
        setEvent(found ?? null);
      })
      .catch((e) => setError(errorMessage(e)));
  }, [eventId, router]);

  async function toggle(body: { status?: string; sales_closed?: boolean }) {
    setError(null);
    try {
      setEvent(await updateEvent(eventId, body));
    } catch (e) {
      setError(errorMessage(e));
    }
  }
  async function remove() {
    if (!confirm("Скасувати подію? Її буде прибрано з афіші назавжди.")) return;
    try {
      await cancelEvent(eventId);
      router.push("/organizer");
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  if (error && !event) return <main className="container narrow"><div className="error">{error}</div></main>;
  if (!event) return <main className="container narrow"><p className="muted">Завантаження…</p></main>;

  const initial: EventFormInitial = {
    title: event.title,
    description: event.description ?? "",
    category: event.category,
    city: event.city ?? "",
    venue_address: event.venue_address ?? "",
    starts_at_local: event.starts_at ? event.starts_at.slice(0, 16) : "",
    price: event.price,
    tickets_total: String(event.tickets_left),
    is_free: event.cost_type === "free",
    poster_url: event.poster_url,
    video_url: event.video_url ?? "",
    links: event.links.join(", "),
    is_draft: event.status === "draft",
    age_rating: event.age_rating ?? "",
  };

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24 }}>Редагування події</h1>
      {error && <div className="error">{error}</div>}
      <div className="row gap" style={{ flexWrap: "wrap", margin: "12px 0" }}>
        {event.status === "published"
          ? <button className="btn sm" onClick={() => toggle({ status: "hidden" })}>Сховати з афіші</button>
          : <button className="btn sm" onClick={() => toggle({ status: "published" })}>{event.status === "draft" ? "Опублікувати чернетку" : "Опублікувати"}</button>}
        <button className="btn sm" onClick={() => toggle({ sales_closed: !event.sales_closed })}>
          {event.sales_closed ? "Відновити продаж" : "Стоп продажів"}
        </button>
        <button className="btn sm danger" onClick={remove}>Скасувати подію</button>
        <span className="pill" style={{ marginLeft: "auto" }}>{event.status}</span>
      </div>
      <EventForm
        initial={initial}
        submitLabel="Зберегти зміни"
        onSubmit={async (f) => {
          await updateEvent(eventId, toEventInput(f, toIso(f.starts_at_local)));
          router.push("/organizer");
        }}
      />
    </main>
  );
}
