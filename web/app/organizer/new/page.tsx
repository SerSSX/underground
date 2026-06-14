"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import EventForm, { EMPTY_EVENT, toEventInput } from "../../components/EventForm";
import { createEvent, getToken, toIso } from "../../../lib/api";

export default function NewEventPage() {
  const router = useRouter();
  useEffect(() => {
    if (!getToken()) router.push("/login?next=/organizer/new");
  }, [router]);

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24 }}>Нова подія</h1>
      <EventForm
        initial={EMPTY_EVENT}
        submitLabel="Опублікувати"
        allowDraft
        onSubmit={async (f) => {
          await createEvent({ ...toEventInput(f, toIso(f.starts_at_local)), status: f.is_draft ? "draft" : "published" });
          router.push("/organizer");
        }}
      />
    </main>
  );
}
