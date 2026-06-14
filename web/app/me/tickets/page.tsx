"use client";

import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import QRCode from "qrcode";
import {
  myTickets, returnTicket, transferTicket, disputeOrder, getToken, errorMessage, type TicketDto,
} from "../../../lib/api";
import { useLang, tr, fmtDateL } from "../../../lib/i18n";
import EscrowTimeline from "../../components/EscrowTimeline";
import Modal from "../../components/Modal";

const STATUS: Record<string, { uk: string; en: string; cls: string }> = {
  active: { uk: "✓ Активний", en: "✓ Active", cls: "pill ok" },
  used: { uk: "• Використаний", en: "• Used", cls: "pill" },
  returned: { uk: "✕ Повернений", en: "✕ Returned", cls: "pill bad" },
};

function MyTicketsInner() {
  const router = useRouter();
  const { lang } = useLang();
  const newId = useSearchParams().get("new");
  const [tickets, setTickets] = useState<TicketDto[]>([]);
  const [qr, setQr] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [transferFor, setTransferFor] = useState<string | null>(null);
  const [recipient, setRecipient] = useState("");
  const [confirmState, setConfirmState] = useState<
    { title: string; body: string; cta: string; run: () => Promise<void> } | null
  >(null);

  async function load() {
    const list = await myTickets();
    setTickets(list);
    const map: Record<string, string> = {};
    for (const t of list) {
      if (t.status === "active") map[t.id] = await QRCode.toDataURL(t.qr_token, { margin: 1, width: 220 });
    }
    setQr(map);
  }

  useEffect(() => {
    if (!getToken()) {
      router.push("/login?next=/me/tickets");
      return;
    }
    load().catch((e) => setError(errorMessage(e))).finally(() => setLoading(false));
  }, [router]);

  function doReturn(id: string) {
    setConfirmState({
      title: tr(lang, "Повернути квиток?", "Return ticket?"),
      body: tr(lang, "Місце звільниться, кошти повернуться (поки не сплинув термін).", "The seat is freed and you're refunded (while still recallable)."),
      cta: tr(lang, "Повернути", "Return"),
      run: async () => {
        setBusy(id);
        try {
          await returnTicket(id);
          await load();
          setNotice(tr(lang, "Квиток повернено, кошти повернуться.", "Ticket returned; you'll be refunded."));
        } finally {
          setBusy(null);
        }
      },
    });
  }

  async function doTransfer() {
    const id = transferFor;
    if (!id || !recipient.trim()) return;
    setBusy(id);
    setError(null);
    setNotice(null);
    try {
      await transferTicket(id, recipient.trim());
      setTransferFor(null);
      setRecipient("");
      await load();
      setNotice(tr(lang, "Квиток подаровано.", "Ticket transferred."));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  function doDispute(ticketId: string, orderId: string) {
    setConfirmState({
      title: tr(lang, "Відкрити спір?", "Open a dispute?"),
      body: tr(lang, "Кошти заморозяться до рішення модерації, виплата організатору призупиниться.", "Funds are frozen until moderation decides; the organizer payout is paused."),
      cta: tr(lang, "Відкрити спір", "Open dispute"),
      run: async () => {
        setBusy(ticketId);
        try {
          await disputeOrder(orderId);
          await load();
          setNotice(tr(lang, "Спір відкрито. Модерація розгляне звернення.", "Dispute opened. Moderation will review it."));
        } finally {
          setBusy(null);
        }
      },
    });
  }

  async function runConfirm() {
    if (!confirmState) return;
    setError(null);
    setNotice(null);
    const action = confirmState;
    setConfirmState(null);
    try {
      await action.run();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24 }}>{tr(lang, "Мої квитки", "My tickets")}</h1>
      {error && <div className="error">{error}</div>}
      {notice && <div className="pill ok" style={{ display: "block", marginBottom: 8 }}>{notice}</div>}
      {loading && <p className="muted">{tr(lang, "Завантаження…", "Loading…")}</p>}
      {!loading && tickets.length === 0 && <p className="muted">{tr(lang, "Ще немає квитків. Оберіть подію в афіші.", "No tickets yet. Pick an event from the listings.")}</p>}
      <div className="grid">
        {tickets.map((t) => {
          const st = STATUS[t.status] ?? { uk: t.status, en: t.status, cls: "pill" };
          const isNew = t.id === newId;
          return (
            <div key={t.id} className="ticket" style={isNew ? { outline: "2px solid var(--accent)", borderRadius: 12 } : undefined}>
              {isNew && <div className="pill ok" style={{ marginBottom: 8 }}>{tr(lang, "✓ Новий квиток", "✓ New ticket")}</div>}
              <h3 style={{ margin: "0 0 4px" }}>{t.event_title}</h3>
              <div className="muted" style={{ fontSize: 13, marginBottom: 12 }}>
                {[t.event_city, fmtDateL(t.event_starts_at, lang)].filter(Boolean).join(" · ")}
              </div>
              {t.status === "active" && qr[t.id]
                ? <img src={qr[t.id]} alt={tr(lang, "QR квитка", "Ticket QR")} />
                : <div className="muted" style={{ padding: 40 }}>{tr(lang, "QR недоступний", "QR unavailable")}</div>}
              <div style={{ marginTop: 12 }}><span className={st.cls}>{tr(lang, st.uk, st.en)}</span></div>
              {t.status === "active" && (
                <div className="row gap" style={{ marginTop: 10 }}>
                  <button className="btn ghost sm" onClick={() => doReturn(t.id)} disabled={busy === t.id}>
                    {tr(lang, "Повернути", "Return")}
                  </button>
                  {t.escrow_status !== "frozen" && (
                    <button className="btn ghost sm" onClick={() => { setRecipient(""); setTransferFor(t.id); }} disabled={busy === t.id}>
                      {tr(lang, "Подарувати", "Transfer")}
                    </button>
                  )}
                </div>
              )}
              {/* Receipt + escrow lifecycle for paid tickets (WEB-2/3/4). */}
              {t.order_id && (
                <div style={{ marginTop: 12, borderTop: "1px solid var(--border)", paddingTop: 10 }}>
                  <div className="row spread" style={{ fontSize: 12 }}>
                    <span className="muted">{tr(lang, "Замовлення", "Order")} #{t.order_id.slice(0, 8)}</span>
                    <span className="muted">{fmtDateL(t.created_at, lang)}</span>
                  </div>
                  <div className="row spread" style={{ fontSize: 13, marginTop: 4 }}>
                    <span>{tr(lang, "Сплачено", "Paid")}: <strong>{t.price} ₴</strong></span>
                  </div>
                  <EscrowTimeline status={t.escrow_status} />
                  {(t.escrow_status === "held" || t.escrow_status === "clearing") && (
                    <button className="btn ghost sm" style={{ marginTop: 10 }} onClick={() => doDispute(t.id, t.order_id!)} disabled={busy === t.id}>
                      {tr(lang, "Поскаржитися на подію", "Dispute this order")}
                    </button>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {confirmState && (
        <Modal title={confirmState.title} onClose={() => setConfirmState(null)}>
          <p className="muted">{confirmState.body}</p>
          <div className="row gap" style={{ marginTop: 16, justifyContent: "flex-end" }}>
            <button className="btn ghost" onClick={() => setConfirmState(null)}>{tr(lang, "Скасувати", "Cancel")}</button>
            <button className="btn" onClick={runConfirm}>{confirmState.cta}</button>
          </div>
        </Modal>
      )}

      {transferFor && (
        <Modal title={tr(lang, "Подарувати квиток", "Transfer ticket")} onClose={() => setTransferFor(null)}>
          <form className="form" onSubmit={(e) => { e.preventDefault(); doTransfer(); }}>
            <input
              className="input"
              type="email"
              placeholder={tr(lang, "Email отримувача", "Recipient email")}
              value={recipient}
              onChange={(e) => setRecipient(e.target.value)}
              required
              autoComplete="email"
            />
            <p className="muted" style={{ fontSize: 12 }}>{tr(lang, "Отримувач має акаунт underground.", "The recipient needs an underground account.")}</p>
            <button className="btn block" disabled={!recipient.trim() || busy === transferFor}>{tr(lang, "Подарувати", "Transfer")}</button>
          </form>
        </Modal>
      )}
    </main>
  );
}

export default function MyTicketsPage() {
  return (
    <Suspense fallback={<main className="container narrow"><p className="muted">…</p></main>}>
      <MyTicketsInner />
    </Suspense>
  );
}
