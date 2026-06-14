"use client";

import { useState } from "react";
import StatusBadge from "./StatusBadge";
import type { Payout } from "../../lib/api";

type Mode = "paid" | "failed" | "pending" | null;

/**
 * Payout settlement drawer (R1 + R3): replaces the browser prompt() for the failure reason and
 * adds an explicit confirmation step (organizer + amount + current→target) before a money status
 * change, so a misclick can't irreversibly settle a payout.
 */
export default function PayoutActionDrawer({
  payout,
  busy,
  onClose,
  onConfirm,
}: {
  payout: Payout | null;
  busy?: boolean;
  onClose: () => void;
  onConfirm: (status: string, reason?: string) => void;
}) {
  const [mode, setMode] = useState<Mode>(null);
  const [reason, setReason] = useState("");
  if (!payout) return null;
  const close = () => { setMode(null); setReason(""); onClose(); };

  return (
    <div className="drawer-scrim" onClick={close}>
      <div className="drawer" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal>
        <div className="row spread">
          <h2>Виплата</h2>
          <button className="btn ghost sm" onClick={close} aria-label="Закрити">✕</button>
        </div>

        <div>
          <div className="kv"><span className="k">Організатор</span><span className="val">{payout.organizer_email ?? payout.organizer_id}</span></div>
          <div className="kv"><span className="k">Сума</span><span className="val">{payout.amount} {payout.currency}</span></div>
          <div className="kv"><span className="k">Замовлень</span><span className="val">{payout.order_count}</span></div>
          <div className="kv"><span className="k">Статус</span><span className="val"><StatusBadge status={payout.status} /></span></div>
          {payout.failure_reason && <div className="kv"><span className="k">Причина</span><span className="val">{payout.failure_reason}</span></div>}
        </div>

        {mode === "failed" ? (
          <>
            <label className="lbl">Причина невдачі
              <textarea className="input" rows={3} value={reason} onChange={(e) => setReason(e.target.value)} />
            </label>
            <div className="danger-note">⚠ Дія з коштами на {payout.amount} {payout.currency}. Перевірте суму та організатора.</div>
            <div className="actionstack">
              <button className="btn danger" disabled={busy || !reason.trim()} onClick={() => onConfirm("failed", reason.trim())}>Підтвердити «невдало»</button>
              <button className="btn ghost" onClick={() => setMode(null)}>Назад</button>
            </div>
          </>
        ) : mode ? (
          <>
            <div className="danger-note">Змінити статус виплати на <b>{mode === "paid" ? "виплачено" : "очікує"}</b>? Дія з коштами на {payout.amount} {payout.currency} — перевірте організатора.</div>
            <div className="actionstack">
              <button className="btn" disabled={busy} onClick={() => onConfirm(mode)}>Підтвердити</button>
              <button className="btn ghost" onClick={() => setMode(null)}>Назад</button>
            </div>
          </>
        ) : (
          <div className="actionstack">
            {payout.status !== "paid" && <button className="btn ok" onClick={() => setMode("paid")}>Позначити «виплачено»</button>}
            {payout.status !== "failed" && <button className="btn danger" onClick={() => setMode("failed")}>Позначити «невдало»</button>}
            {payout.status === "failed" && <button className="btn neutral" onClick={() => setMode("pending")}>Повторити (→ очікує)</button>}
          </div>
        )}
      </div>
    </div>
  );
}
