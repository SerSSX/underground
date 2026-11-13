"use client";

import { useLang, tr } from "../../lib/i18n";

// Visual escrow lifecycle for a paid ticket: Paid → Event → Released to organizer.
// Explains what each state means (WEB-3 dispute feedback + WEB-4 escrow explanation) and
// surfaces dispute/refund states clearly. Renders nothing for free tickets (escrow "none").
export default function EscrowTimeline({ status }: { status: string | null }) {
  const { lang } = useLang();
  if (!status || status === "none") return null;

  const steps = [
    tr(lang, "Сплачено", "Paid"),
    tr(lang, "Подія", "Event"),
    tr(lang, "Кошти організатору", "Released"),
  ];
  // How far the lifecycle has progressed (index of the last completed node).
  const reached =
    status === "held" ? 0 : status === "clearing" ? 1 : status === "cleared" || status === "paid_out" ? 2 : 0;

  const note: Record<string, { text: string; tone: string }> = {
    held: { text: tr(lang, "Кошти захищені в ескроу до завершення події.", "Funds are protected in escrow until the event ends."), tone: "var(--muted)" },
    clearing: { text: tr(lang, "Подія відбулась — кошти очищуються перед виплатою.", "Event is over — funds are clearing before payout."), tone: "var(--muted)" },
    cleared: { text: tr(lang, "Кошти готові до виплати організатору.", "Funds are cleared for the organizer payout."), tone: "var(--ok-fg, #5fd08a)" },
    paid_out: { text: tr(lang, "Виплачено організатору.", "Paid out to the organizer."), tone: "var(--ok-fg, #5fd08a)" },
    frozen: { text: tr(lang, "⏳ Спір на розгляді — кошти заморожено до рішення модерації.", "⏳ Dispute under review — funds frozen pending moderation."), tone: "var(--warn-fg, #e6b860)" },
    refunded: { text: tr(lang, "↩ Кошти повернено покупцю.", "↩ Refunded to the buyer."), tone: "var(--danger-fg, #f0816a)" },
  };
  const n = note[status];
  const disputed = status === "frozen" || status === "refunded";

  return (
    <div style={{ marginTop: 12, padding: 12, background: "var(--surface2, #1b1b21)", borderRadius: 12, border: "1px solid var(--border)" }}>
      {!disputed && (
        <div className="row" style={{ gap: 0, marginBottom: 8 }}>
          {steps.map((label, i) => (
            <div key={label} style={{ flex: 1, textAlign: "center", position: "relative" }}>
              <div
                style={{
                  width: 18, height: 18, borderRadius: 999, margin: "0 auto 4px",
                  background: i <= reached ? "var(--accent)" : "var(--border)",
                  color: "#fff", fontSize: 11, lineHeight: "18px",
                }}
              >
                {i <= reached ? "✓" : i + 1}
              </div>
              <div style={{ fontSize: 11, color: i <= reached ? "var(--text)" : "var(--muted)" }}>{label}</div>
            </div>
          ))}
        </div>
      )}
      {n && <div style={{ fontSize: 12, color: n.tone }}>{n.text}</div>}
    </div>
  );
}
