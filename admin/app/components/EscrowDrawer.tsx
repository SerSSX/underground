"use client";

import { useState } from "react";
import StatusBadge from "./StatusBadge";
import type { AdminOrder } from "../../lib/api";

type EscrowAction = "refund" | "freeze" | "release";

/**
 * Right-side drawer for the highest-risk surface (moving real money). Shows a read-only order
 * summary, visually stratifies actions (Refund=danger, Release=primary, Freeze=neutral), gates
 * force-release of a disputed order behind an explicit checkbox, and states that money actions
 * are irreversible — replacing the old bare confirm().
 */
export default function EscrowDrawer({
  order,
  busy,
  onClose,
  onAction,
}: {
  order: AdminOrder | null;
  busy?: boolean;
  onClose: () => void;
  onAction: (action: EscrowAction, force: boolean) => void;
}) {
  const [force, setForce] = useState(false);
  if (!order) return null;
  const es = order.escrow_status;
  const terminal = es === "paid_out" || es === "refunded" || !es;
  const needsForce = es === "frozen";
  const canRelease = es !== "cleared";

  return (
    <div className="drawer-scrim" onClick={onClose}>
      <div className="drawer" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal>
        <div className="row spread">
          <h2>Керування escrow</h2>
          <button className="btn ghost sm" onClick={onClose} aria-label="Закрити">✕</button>
        </div>

        <div>
          <div className="kv"><span className="k">Подія</span><span className="val">{order.event_title ?? "—"}</span></div>
          <div className="kv"><span className="k">Покупець</span><span className="val">{order.buyer_email ?? "—"}</span></div>
          <div className="kv"><span className="k">Сума</span><span className="val">{order.amount} {order.currency}</span></div>
          <div className="kv"><span className="k">Статус</span><span className="val"><StatusBadge status={order.status} /></span></div>
          <div className="kv"><span className="k">Escrow</span><span className="val"><StatusBadge status={es} /></span></div>
        </div>

        {terminal ? (
          <div className="danger-note">
            Кошти вже {es === "paid_out" ? "виплачені організатору" : es === "refunded" ? "повернені покупцю" : "не залучені"} — дій немає.
          </div>
        ) : (
          <>
            {needsForce && (
              <label className="checkrow">
                <input type="checkbox" checked={force} onChange={(e) => setForce(e.target.checked)} />
                <span>Це замовлення <b>СПІРНЕ (frozen)</b>. Підтверджую примусове рішення на користь організатора.</span>
              </label>
            )}
            <div className="danger-note">⚠ Дії з коштами незворотні. Перевірте суму та сторони перед підтвердженням.</div>
            <div className="actionstack">
              <button className="btn danger" disabled={busy} onClick={() => onAction("refund", false)}>↩ Повернути покупцю (refund)</button>
              {canRelease && (
                <button className="btn" disabled={busy || (needsForce && !force)} onClick={() => onAction("release", needsForce)}>
                  ✓ Звільнити організатору{needsForce ? " (force)" : ""}
                </button>
              )}
              {es !== "frozen" && (
                <button className="btn neutral" disabled={busy} onClick={() => onAction("freeze", false)}>❄ Заморозити (freeze)</button>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
