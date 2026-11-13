"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { getEvent, getProfile, getToken, isOver, mySaves, myTickets, type EventDto } from "../../lib/api";
import { useLang, tr, tf } from "../../lib/i18n";
import { initials } from "../../lib/ui";
import { MessageCircle } from "../components/Icon";

/** One organiser the user can write to, and the event the first message is about. */
type Contact = { id: string; name: string; avatar: string | null; eventId: string; eventTitle: string; from: "ticket" | "saved" };

/** At most this many events are looked up for their organiser, and this many organisers are shown. */
const MAX_EVENTS = 8;
const MAX_CONTACTS = 6;

type Candidate = { organizerId: string; eventId: string; title: string; from: "ticket" | "saved" };

/** Upcoming events behind the user's live tickets (soonest first), then their upcoming saved events. */
async function candidates(): Promise<Candidate[]> {
  const [tickets, saves] = await Promise.all([myTickets().catch(() => []), mySaves().catch(() => [] as EventDto[])]);
  const live = tickets
    .filter((t) => (t.status === "active" || t.status === "pending_claim") && !isOver(t.event_starts_at, t.event_ends_at))
    .sort((a, b) => (a.event_starts_at ?? "9").localeCompare(b.event_starts_at ?? "9"));
  const ticketEventIds = [...new Set(live.map((t) => t.event_id))].slice(0, MAX_EVENTS);
  // Tickets do not carry the organiser, so each event is read once.
  const ticketEvents = await Promise.all(ticketEventIds.map((id) => getEvent(id).catch(() => null)));
  const out: Candidate[] = [];
  for (const ev of ticketEvents) {
    if (ev?.organizer_id) out.push({ organizerId: ev.organizer_id, eventId: ev.id, title: ev.title, from: "ticket" });
  }
  for (const ev of saves) {
    if (ev.organizer_id && !isOver(ev.starts_at, ev.ends_at)) out.push({ organizerId: ev.organizer_id, eventId: ev.id, title: ev.title, from: "saved" });
  }
  return out;
}

/** One row per organiser (its first event wins), never the user themselves. */
async function contacts(meId: string | null): Promise<Contact[]> {
  const seen = new Set<string>();
  const picked: Candidate[] = [];
  for (const c of await candidates()) {
    if (seen.has(c.organizerId) || c.organizerId === meId) continue;
    seen.add(c.organizerId);
    picked.push(c);
    if (picked.length >= MAX_CONTACTS) break;
  }
  const rows = await Promise.all(picked.map(async (c) => {
    try {
      const p = await getProfile(c.organizerId);
      return { id: p.id, name: p.name, avatar: p.avatar_url, eventId: c.eventId, eventTitle: c.title, from: c.from } satisfies Contact;
    } catch {
      return null;
    }
  }));
  return rows.filter((r): r is Contact => r !== null);
}

/**
 * "Start a conversation": organisers of the user's upcoming tickets and saved events, each with a Message button
 * that opens /inbox?to=<organiser>&about=<event> (TASK-305). People already in the conversation list (`skip`) and
 * the user themselves are left out. Renders nothing when there is nobody to offer.
 */
export default function StartChats({ meId, skip, variant }: { meId: string | null; skip: ReadonlySet<string>; variant: "card" | "list" }) {
  const { lang } = useLang();
  const [list, setList] = useState<Contact[] | null>(null);

  useEffect(() => {
    if (!getToken()) return;
    let stop = false;
    contacts(meId).then((l) => { if (!stop) setList(l); }).catch(() => { if (!stop) setList([]); });
    return () => { stop = true; };
  }, [meId]);

  const shown = (list ?? []).filter((c) => c.id !== meId && !skip.has(c.id));
  if (shown.length === 0) return null;

  const rows = shown.map((c) => (
    <div key={c.id} className="chat-item start-chat">
      <Link href={`/u/${c.id}`} className="chat-peer grow" aria-label={tf(lang, "Профіль: {name}", "Profile: {name}", { name: c.name })}>
        <span className="avatar sm">{c.avatar ? <img src={c.avatar} alt="" /> : initials(c.name)}</span>
        <span className="grow" style={{ minWidth: 0 }}>
          <span className="strong" style={{ display: "block" }}>{c.name}</span>
          <span className="muted xs" style={{ display: "block", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
            {c.from === "ticket"
              ? tf(lang, "Квиток: {title}", "Ticket: {title}", { title: c.eventTitle })
              : tf(lang, "Збережено: {title}", "Saved: {title}", { title: c.eventTitle })}
          </span>
        </span>
      </Link>
      <Link className="btn secondary sm" href={`/inbox?to=${c.id}&about=${c.eventId}`}>
        <MessageCircle className="ico sm" aria-hidden />{tr(lang, "Написати", "Message")}
      </Link>
    </div>
  ));

  if (variant === "list") {
    return (
      <div className="start-chats" aria-label={tr(lang, "Почати розмову", "Start a conversation")} role="group">
        <div className="muted xs strong start-chats-head">{tr(lang, "Почати розмову", "Start a conversation")}</div>
        {rows}
      </div>
    );
  }
  return (
    <section className="card start-chats" style={{ marginBottom: 16, padding: 6 }}>
      <div style={{ padding: "12px 16px 4px" }}>
        <h3 style={{ fontSize: 16 }}>{tr(lang, "Почати розмову", "Start a conversation")}</h3>
        <p className="muted small">{tr(lang, "Організатори подій, на які ви йдете або які зберегли.", "Organisers of events you're going to or have saved.")}</p>
      </div>
      {rows}
    </section>
  );
}
