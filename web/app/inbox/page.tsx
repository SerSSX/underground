"use client";

import Link from "next/link";
import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import {
  myNotifications, markNotificationsRead, myConversations, messageThread, sendMessage, getProfile, getEvent, getToken, errorMessage,
  isUuid, alertHref, type NotificationDto, type ConversationDto, type MessageDto, type EventDto,
} from "../../lib/api";
import { createSendQueue, stashFailed, takeDraft } from "../../lib/send-queue.mjs";
import { unreadChanged } from "../../lib/unread";
import { useLang, tr, tf } from "../../lib/i18n";
import { useSession } from "../../lib/session";
import { initials, shortWhen, timeOf } from "../../lib/ui";
import { isNearBottom } from "../../lib/chat-scroll.mjs";
import { ChevronLeft, MessageCircle, Send, Bell, Calendar } from "../components/Icon";
import StartChats from "./StartChats";

type Peer = { id: string; name: string; avatar: string | null };
/** One message waiting to go: who it is for and what it says. */
type Outgoing = { peerId: string; body: string };
/** The event a chat was opened from (`about=`), shown as a chip above that conversation's messages. */
type About = { peerId: string; event: Pick<EventDto, "id" | "title" | "starts_at" | "timezone"> };

function InboxInner() {
  const router = useRouter();
  const params = useSearchParams();
  const { lang } = useLang();
  const { me } = useSession();
  const [tab, setTab] = useState<"chats" | "alerts">("chats");
  const [convs, setConvs] = useState<ConversationDto[] | null>(null);
  const [alerts, setAlerts] = useState<NotificationDto[] | null>(null);
  const [peer, setPeer] = useState<Peer | null>(null);
  const [msgs, setMsgs] = useState<MessageDto[]>([]);
  const [text, setText] = useState("");
  // The field's text as of the last keystroke or send, read by send(): two submits before React re-renders (a
  // double tap) must not both see the same text and queue it twice.
  const textRef = useRef("");
  useEffect(() => { textRef.current = text; }, [text]);
  const [about, setAbout] = useState<About | null>(null);
  const [error, setError] = useState<string | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  // The "About «title»: " text put in the field when the chat is opened from an event; while the field still holds
  // exactly that, focusing it puts the caret at the end so typing continues the sentence.
  const prefill = useRef<string | null>(null);
  // Whether the reader is at (within 80 px of) the newest message. Kept up to date on every scroll of the list,
  // so a keyboard opening or closing only follows the newest message for someone who was already there.
  const nearBottom = useRef(true);

  const loadConvs = useCallback(() => myConversations().then(setConvs).catch((e) => { setError(errorMessage(e)); setConvs([]); }), []);

  useEffect(() => {
    if (!getToken()) {
      // Keep the deep link (who, and which event) through sign-in.
      const keep = new URLSearchParams();
      for (const k of ["to", "about"]) { const v = params.get(k); if (isUuid(v)) keep.set(k, v); }
      const q = keep.toString();
      router.replace(`/login?next=${encodeURIComponent("/inbox" + (q ? `?${q}` : ""))}`);
      return;
    }
    void loadConvs();
    myNotifications().then(setAlerts).catch(() => setAlerts([]));
  }, [router, params, loadConvs]);

  // Deep link from an event page, a profile or the "start a conversation" list: /inbox?to=<user id>&about=<event id>
  // opens (or starts) that conversation. The key of the last link applied is kept until the URL drops it, so the
  // same page can open a second deep link later (the start list links back to /inbox).
  const applied = useRef<string | null>(null);
  useEffect(() => {
    const to = params.get("to");
    if (!to) { applied.current = null; return; }
    const about = params.get("about");
    const key = `${to}|${about ?? ""}`;
    if (!convs || applied.current === key) return;
    applied.current = key;
    router.replace("/inbox");
    if (!isUuid(to)) return; // only a user id, never a path into the API
    if (me && to === me.id) return; // nobody writes to themselves (the API refuses it too)
    if (isUuid(about)) {
      // Start the message with the event it is about, so the organiser knows the context, and show that event above
      // the messages (TASK-305).
      getEvent(about).then((ev) => {
        setAbout({ peerId: to, event: { id: ev.id, title: ev.title, starts_at: ev.starts_at, timezone: ev.timezone } });
        const start = tf(lang, "Щодо «{title}»: ", "About “{title}”: ", { title: ev.title });
        setText((t) => {
          if (t) return t;
          prefill.current = start;
          return start;
        });
      }).catch(() => {});
    }
    const c = convs.find((x) => x.peer_id === to);
    if (c) setPeer({ id: c.peer_id, name: c.peer_name, avatar: c.peer_avatar });
    else getProfile(to).then((p) => setPeer({ id: p.id, name: p.name, avatar: p.avatar_url })).catch(() => {});
  }, [params, convs, me, lang, router]);

  useEffect(() => {
    setMsgs([]);
    if (!peer) return;
    let stop = false;
    const pull = () => messageThread(peer.id).then((m) => {
      if (stop) return;
      setMsgs(m);
      // Reading the thread marked these read on the server: the list's count and the nav badges catch up now.
      if (m.some((x) => !x.read && x.sender_id === peer.id)) {
        unreadChanged();
        void loadConvs();
      }
    }).catch(() => {});
    void pull();
    const id = setInterval(pull, 8000);
    return () => { stop = true; clearInterval(id); };
  }, [peer, loadConvs]);
  // Newest message into view by scrolling the message list itself, never the page: scrollIntoView also scrolled
  // the window, which on a phone pushed the compose bar under the on-screen keyboard (TASK-304).
  const toNewest = useCallback(() => {
    const el = list.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, []);
  useEffect(toNewest, [msgs.length, toNewest]);

  // Phones: an open conversation fills exactly the VISIBLE part of the screen (the visual viewport, which shrinks
  // when the keyboard opens), so the compose bar sits right above the keyboard and the last message above it.
  // The page behind is locked so it cannot scroll the bar away.
  useEffect(() => {
    if (!peer) return;
    const root = document.documentElement;
    const vv = window.visualViewport;
    const fit = () => {
      root.style.setProperty("--vv-h", `${Math.round(vv ? vv.height : window.innerHeight)}px`);
      root.style.setProperty("--vv-top", `${Math.round(vv ? vv.offsetTop : 0)}px`);
      // Only someone already at the newest message is kept there; a reader of older messages stays put.
      if (nearBottom.current) toNewest();
    };
    nearBottom.current = true; // a conversation opens at its newest message
    fit();
    root.classList.add("chat-open");
    vv?.addEventListener("resize", fit);
    vv?.addEventListener("scroll", fit);
    window.addEventListener("resize", fit);
    return () => {
      vv?.removeEventListener("resize", fit);
      vv?.removeEventListener("scroll", fit);
      window.removeEventListener("resize", fit);
      root.classList.remove("chat-open");
      root.style.removeProperty("--vv-h");
      root.style.removeProperty("--vv-top");
    };
  }, [peer, toNewest]);

  useEffect(() => {
    if (tab === "alerts" && alerts?.some((n) => !n.read)) void markNotificationsRead().then(unreadChanged).catch(() => {});
  }, [tab, alerts]);

  // Unsent texts of conversations that were not open when their send failed, given back when they are opened.
  const unsent = useRef(new Map<string, string>());
  useEffect(() => {
    if (!peer) return;
    const kept = takeDraft(unsent.current, peer.id);
    if (kept) setText((t) => (t.trim() ? `${kept} ${t}` : kept));
  }, [peer]);

  // The open conversation, for the send queue's callbacks (they outlive the render that created them).
  const peerRef = useRef<Peer | null>(null);
  peerRef.current = peer;
  // Messages go one at a time and in order: a Send tap while one is in flight is queued and goes right after it,
  // never dropped and never sent twice (TASK-305 review; lib/send-queue.mjs).
  const outbox = useRef<ReturnType<typeof createSendQueue<Outgoing, MessageDto>> | null>(null);
  if (!outbox.current) {
    outbox.current = createSendQueue<Outgoing, MessageDto>({
      send: (o) => sendMessage(o.peerId, o.body),
      onSent: (o, m) => {
        // Only into the thread it belongs to (the user may have opened another one meanwhile), and once: the
        // 8-second poll may already have brought it.
        if (peerRef.current?.id === o.peerId) setMsgs((prev) => (prev.some((x) => x.id === m.id) ? prev : [...prev, m]));
        void loadConvs();
      },
      onFailed: (failed, e) => {
        setError(errorMessage(e));
        // Give the unsent text back, oldest first, ahead of anything typed since, so nothing is lost. Texts for
        // another chat (the user switched while they were queued) wait for that chat (TASK-305 round 3).
        const here = stashFailed(unsent.current, failed, peerRef.current?.id);
        if (here) setText((t) => (t.trim() ? `${here} ${t}` : here));
      },
    });
  }

  function send() {
    const body = textRef.current.trim();
    if (!peer || !body) return;
    setError(null);
    // Clear at once, so what the user types while this one is in flight is kept (TASK-304 DEV check).
    textRef.current = "";
    setText("");
    prefill.current = null;
    outbox.current?.enqueue({ peerId: peer.id, body });
  }

  // Send button and the keyboard's Send key both land here. The field is focused again synchronously, inside the
  // tap's own event and before any await: iOS only lets a script focus a field (and keep the keyboard up) from a
  // user gesture. The button used to keep focus with pointerdown preventDefault, which in WebKit with touch also
  // swallowed the tap, so the button did nothing on iPhones (TASK-305).
  function onCompose(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault();
    input.current?.focus();
    send();
  }

  // Caret to the end of an untouched prefill. Deferred one tick: on a phone the tap places the caret where the finger
  // landed right after focus, which would otherwise win.
  function onFieldFocus(e: React.FocusEvent<HTMLInputElement>) {
    const el = e.currentTarget;
    if (prefill.current !== null && el.value === prefill.current) {
      const end = el.value.length;
      el.setSelectionRange(end, end);
      setTimeout(() => { if (document.activeElement === el && el.value === prefill.current) el.setSelectionRange(end, end); }, 0);
    }
    setTimeout(toNewest, 300);
  }

  const unreadAlerts = alerts?.filter((n) => !n.read).length ?? 0;
  const unreadChats = (convs ?? []).reduce((a, c) => a + c.unread, 0);
  const peerIds = new Set((convs ?? []).map((c) => c.peer_id));

  return (
    <main className="container">
      <div className="page-head"><h1>{tr(lang, "Повідомлення", "Messages")}</h1></div>
      <div className="tabs" role="tablist">
        <button role="tab" aria-selected={tab === "chats"} className={tab === "chats" ? "active" : ""} onClick={() => setTab("chats")}><MessageCircle className="ico sm" />{tr(lang, "Чати", "Chats")}{unreadChats > 0 && <span className="badge-dot">{unreadChats > 99 ? "99+" : unreadChats}</span>}</button>
        <button role="tab" aria-selected={tab === "alerts"} className={tab === "alerts" ? "active" : ""} onClick={() => setTab("alerts")}><Bell className="ico sm" />{tr(lang, "Сповіщення", "Alerts")}{unreadAlerts > 0 && <span className="badge-dot">{unreadAlerts}</span>}</button>
      </div>
      {error && <div className="alert error" style={{ marginBottom: 12 }}>{error}</div>}

      {tab === "chats" ? (
        convs === null ? (
          <div className="skeleton" style={{ height: 420 }} />
        ) : convs.length === 0 && !peer ? (
          <>
            <StartChats meId={me?.id ?? null} skip={peerIds} variant="card" />
            <div className="empty">
              <div className="ei"><MessageCircle className="ico lg" /></div>
              <h3>{tr(lang, "Поки без розмов", "No conversations yet")}</h3>
              <p>{tr(lang, "Маєте питання до організатора? Натисніть «Написати» на сторінці події.", "Got a question for an organiser? Tap Message on the event page.")}</p>
              <Link className="btn" href="/">{tr(lang, "До афіші", "Browse events")}</Link>
            </div>
          </>
        ) : (
          <div className={`chat-layout ${peer ? "has-thread" : ""}`}>
            <div className="chat-list">
              <StartChats meId={me?.id ?? null} skip={peerIds} variant="list" />
              {convs.map((c) => (
                <button key={c.peer_id} className={`chat-item ${peer?.id === c.peer_id ? "active" : ""}`} onClick={() => setPeer({ id: c.peer_id, name: c.peer_name, avatar: c.peer_avatar })}>
                  <span className="avatar">{c.peer_avatar ? <img src={c.peer_avatar} alt="" /> : initials(c.peer_name)}</span>
                  <span className="grow">
                    <span className="row spread"><span className="strong">{c.peer_name}</span><span className="subtle xs">{timeOf(c.last_at)}</span></span>
                    <span className="muted small" style={{ display: "block", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{c.last_from_me ? tr(lang, "Ви: ", "You: ") : ""}{c.last_text}</span>
                  </span>
                  {c.unread > 0 && <span className="badge-dot">{c.unread}</span>}
                </button>
              ))}
            </div>
            <div className="chat-thread">
              {peer ? (
                <>
                  <div className="chat-head">
                    <button className="btn quiet icon sm" onClick={() => setPeer(null)} aria-label={tr(lang, "До списку", "Back to list")}><ChevronLeft className="ico" /></button>
                    <Link href={`/u/${peer.id}`} className="chat-peer" aria-label={tf(lang, "Профіль: {name}", "Profile: {name}", { name: peer.name })}>
                      <span className="avatar sm">{peer.avatar ? <img src={peer.avatar} alt="" /> : initials(peer.name)}</span>
                      <span className="strong">{peer.name}</span>
                    </Link>
                  </div>
                  {about && about.peerId === peer.id && (
                    <Link href={`/events/${about.event.id}`} className="chat-about" aria-label={tf(lang, "Подія: {title}", "Event: {title}", { title: about.event.title })}>
                      <Calendar className="ico sm" aria-hidden />
                      <span className="strong chat-about-title">{about.event.title}</span>
                      {about.event.starts_at && <span className="muted xs">{shortWhen(about.event.starts_at, lang, about.event.timezone)}</span>}
                    </Link>
                  )}
                  <div className="chat-msgs" aria-live="polite" ref={list} onScroll={(e) => { const el = e.currentTarget; nearBottom.current = isNearBottom(el.scrollHeight, el.scrollTop, el.clientHeight); }}>
                    {msgs.length === 0 && <p className="muted small" style={{ textAlign: "center", marginTop: 20 }}>{tr(lang, "Напишіть перше повідомлення — організатор побачить його тут.", "Write the first message — the organiser will see it here.")}</p>}
                    {msgs.map((m) => (
                      <div key={m.id} className={`bubble ${m.sender_id === me?.id ? "mine" : "theirs"}`}>
                        {m.text}<span className="tm">{shortWhen(m.created_at, lang)}</span>
                      </div>
                    ))}
                  </div>
                  <form className="chat-compose" onSubmit={onCompose}>
                    <label className="sr-only" htmlFor="msg">{tr(lang, "Повідомлення", "Message")}</label>
                    <input id="msg" ref={input} className="input" value={text} maxLength={2000} onChange={(e) => { textRef.current = e.target.value; setText(e.target.value); }} onFocus={onFieldFocus} placeholder={tr(lang, "Повідомлення…", "Message…")} autoComplete="off" enterKeyHint="send" />
                    {/* Disabled only while empty: a disabled default button also blocks the keyboard's Send key. */}
                    <button type="submit" className="btn icon" disabled={!text.trim()} aria-label={tr(lang, "Надіслати", "Send")} title={tr(lang, "Надіслати", "Send")}><Send className="ico sm" aria-hidden /></button>
                  </form>
                </>
              ) : (
                <div className="empty" style={{ margin: 24, border: 0 }}><p>{tr(lang, "Оберіть розмову зліва", "Pick a conversation")}</p></div>
              )}
            </div>
          </div>
        )
      ) : alerts === null ? (
        <div className="skeleton" style={{ height: 200 }} />
      ) : alerts.length === 0 ? (
        <div className="empty">
          <div className="ei"><Bell className="ico lg" /></div>
          <h3>{tr(lang, "Поки тихо", "All quiet")}</h3>
          <p>{tr(lang, "Тут будуть нагадування про ваші події й новини від організаторів.", "Reminders about your events and organiser news will appear here.")}</p>
        </div>
      ) : (
        <div className="card" style={{ padding: 6 }}>
          {alerts.map((n) => {
            const inner = (
              <>
                <span className="avatar sm" style={{ background: n.read ? "var(--surface-3)" : "var(--accent)" }}><Bell className="ico sm" /></span>
                <span className="grow">
                  <span className="strong" style={{ display: "block" }}>{n.title}</span>
                  {n.body && <span className="muted small" style={{ display: "block" }}>{n.body}</span>}
                  <span className="subtle xs">{shortWhen(n.created_at, lang)}</span>
                </span>
              </>
            );
            const href = alertHref(n);
            return href ? (
              <Link key={n.id} href={href} className="chat-item" style={{ borderRadius: 12 }}>{inner}</Link>
            ) : (
              <div key={n.id} className="chat-item" style={{ cursor: "default", borderRadius: 12 }}>{inner}</div>
            );
          })}
        </div>
      )}
    </main>
  );
}

export default function InboxPage() {
  return (
    <Suspense fallback={<main className="container"><div className="skeleton" style={{ height: 420 }} /></main>}>
      <InboxInner />
    </Suspense>
  );
}
