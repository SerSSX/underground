"use client";

import Link from "next/link";
import { Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import {
  myNotifications, markNotificationsRead, myConversations, messageThread, sendMessage, getProfile, getToken, errorMessage,
  type NotificationDto, type ConversationDto, type MessageDto,
} from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";
import { useSession } from "../../lib/session";
import { initials, shortWhen, timeOf } from "../../lib/ui";
import { ChevronLeft, MessageCircle, Send, Bell } from "../components/Icon";

type Peer = { id: string; name: string; avatar: string | null };

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
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const bottom = useRef<HTMLDivElement>(null);

  const loadConvs = useCallback(() => myConversations().then(setConvs).catch((e) => { setError(errorMessage(e)); setConvs([]); }), []);

  useEffect(() => {
    if (!getToken()) {
      router.replace(`/login?next=${encodeURIComponent("/inbox" + (params.get("to") ? `?to=${params.get("to")}` : ""))}`);
      return;
    }
    void loadConvs();
    myNotifications().then(setAlerts).catch(() => setAlerts([]));
  }, [router, params, loadConvs]);

  // Deep link from an event page: /inbox?to=<organizer id> opens (or starts) that conversation.
  useEffect(() => {
    const to = params.get("to");
    if (!to || !convs) return;
    const c = convs.find((x) => x.peer_id === to);
    if (c) setPeer({ id: c.peer_id, name: c.peer_name, avatar: c.peer_avatar });
    else getProfile(to).then((p) => setPeer({ id: p.id, name: p.name, avatar: p.avatar_url })).catch(() => {});
  }, [params, convs]);

  useEffect(() => {
    if (!peer) return;
    let stop = false;
    const pull = () => messageThread(peer.id).then((m) => { if (!stop) setMsgs(m); }).catch(() => {});
    void pull();
    const id = setInterval(pull, 8000);
    return () => { stop = true; clearInterval(id); };
  }, [peer]);
  useEffect(() => { bottom.current?.scrollIntoView({ block: "end" }); }, [msgs.length]);

  useEffect(() => {
    if (tab === "alerts" && alerts?.some((n) => !n.read)) void markNotificationsRead().catch(() => {});
  }, [tab, alerts]);

  async function send() {
    const body = text.trim();
    if (!peer || !body) return;
    setSending(true);
    setError(null);
    try {
      const m = await sendMessage(peer.id, body);
      setMsgs((prev) => [...prev, m]);
      setText("");
      void loadConvs();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSending(false);
    }
  }

  const unreadAlerts = alerts?.filter((n) => !n.read).length ?? 0;

  return (
    <main className="container">
      <div className="page-head"><h1>{tr(lang, "Повідомлення", "Messages")}</h1></div>
      <div className="tabs" role="tablist">
        <button role="tab" aria-selected={tab === "chats"} className={tab === "chats" ? "active" : ""} onClick={() => setTab("chats")}><MessageCircle className="ico sm" />{tr(lang, "Чати", "Chats")}</button>
        <button role="tab" aria-selected={tab === "alerts"} className={tab === "alerts" ? "active" : ""} onClick={() => setTab("alerts")}><Bell className="ico sm" />{tr(lang, "Сповіщення", "Alerts")}{unreadAlerts > 0 && <span className="badge-dot">{unreadAlerts}</span>}</button>
      </div>
      {error && <div className="alert error" style={{ marginBottom: 12 }}>{error}</div>}

      {tab === "chats" ? (
        convs === null ? (
          <div className="skeleton" style={{ height: 420 }} />
        ) : convs.length === 0 && !peer ? (
          <div className="empty">
            <div className="ei"><MessageCircle className="ico lg" /></div>
            <h3>{tr(lang, "Поки без розмов", "No conversations yet")}</h3>
            <p>{tr(lang, "Маєте питання до організатора? Натисніть «Написати» на сторінці події.", "Got a question for an organiser? Tap Message on the event page.")}</p>
            <Link className="btn" href="/">{tr(lang, "До афіші", "Browse events")}</Link>
          </div>
        ) : (
          <div className={`chat-layout ${peer ? "has-thread" : ""}`}>
            <div className="chat-list">
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
                    <button className="btn quiet icon sm" onClick={() => { setPeer(null); router.replace("/inbox"); }} aria-label={tr(lang, "До списку", "Back to list")}><ChevronLeft className="ico" /></button>
                    <span className="avatar sm">{peer.avatar ? <img src={peer.avatar} alt="" /> : initials(peer.name)}</span>
                    <span className="strong">{peer.name}</span>
                  </div>
                  <div className="chat-msgs" aria-live="polite">
                    {msgs.length === 0 && <p className="muted small" style={{ textAlign: "center", marginTop: 20 }}>{tr(lang, "Напишіть перше повідомлення — організатор побачить його тут.", "Write the first message — the organiser will see it here.")}</p>}
                    {msgs.map((m) => (
                      <div key={m.id} className={`bubble ${m.sender_id === me?.id ? "mine" : "theirs"}`}>
                        {m.text}<span className="tm">{shortWhen(m.created_at, lang)}</span>
                      </div>
                    ))}
                    <div ref={bottom} />
                  </div>
                  <form className="chat-compose" onSubmit={(e) => { e.preventDefault(); void send(); }}>
                    <label className="sr-only" htmlFor="msg">{tr(lang, "Повідомлення", "Message")}</label>
                    <input id="msg" className="input" value={text} maxLength={2000} onChange={(e) => setText(e.target.value)} placeholder={tr(lang, "Повідомлення…", "Message…")} autoComplete="off" />
                    <button className="btn icon" disabled={sending || !text.trim()} aria-label={tr(lang, "Надіслати", "Send")}><Send className="ico sm" /></button>
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
            return n.event_id ? (
              <Link key={n.id} href={`/events/${n.event_id}`} className="chat-item" style={{ borderRadius: 12 }}>{inner}</Link>
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
