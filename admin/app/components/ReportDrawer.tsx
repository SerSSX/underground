"use client";

import { useState } from "react";
import StatusBadge from "./StatusBadge";
import type { Report } from "../../lib/api";

const ACTION_LABEL: Record<string, string> = {
  event: "Заблокувати подію",
  user: "Заблокувати користувача",
  post: "Видалити пост",
  review: "Видалити відгук",
};

/**
 * Trust & Safety triage drawer (ADM-6): full report context, a "similar reports on the same
 * target" pattern signal, a moderator decision-note (captured into the audit trail), and the
 * enforce / reviewed / resolve actions in one place.
 */
export default function ReportDrawer({
  report,
  all,
  busy,
  onClose,
  onEnforce,
  onResolve,
}: {
  report: Report | null;
  all: Report[];
  busy?: boolean;
  onClose: () => void;
  onEnforce: (r: Report, note: string) => void;
  onResolve: (status: string, note: string) => void;
}) {
  const [note, setNote] = useState("");
  if (!report) return null;
  const similar = all.filter((r) => r.target_id === report.target_id && r.id !== report.id);

  return (
    <div className="drawer-scrim" onClick={onClose}>
      <div className="drawer" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal>
        <div className="row spread">
          <h2>Скарга</h2>
          <button className="btn ghost sm" onClick={onClose} aria-label="Закрити">✕</button>
        </div>

        <div>
          <div className="kv"><span className="k">Тип</span><span className="val">{report.target_type}</span></div>
          <div className="kv"><span className="k">Ціль</span><span className="val mono">{report.target_id.slice(0, 12)}…</span></div>
          <div className="kv"><span className="k">Причина</span><span className="val">{report.reason}</span></div>
          <div className="kv"><span className="k">Скаржник</span><span className="val">{report.reporter_email ?? "—"}</span></div>
          <div className="kv"><span className="k">Дата</span><span className="val">{new Date(report.created_at).toLocaleString("uk-UA")}</span></div>
          <div className="kv"><span className="k">Статус</span><span className="val"><StatusBadge status={report.status} /></span></div>
        </div>

        {report.details && <div style={{ fontSize: 13, color: "var(--muted)" }}>{report.details}</div>}

        {similar.length > 0 && (
          <div className="danger-note">⚠ Ще {similar.length} скарг(и) на цю саму ціль у списку — ймовірний патерн.</div>
        )}

        <label className="lbl">
          Нотатка модератора (потрапить у журнал дій)
          <textarea
            className="input"
            rows={3}
            value={note}
            onChange={(e) => setNote(e.target.value)}
            placeholder="Рішення / обґрунтування…"
          />
        </label>

        <div className="actionstack">
          {report.status !== "resolved" && (
            <button className="btn danger" disabled={busy} onClick={() => onEnforce(report, note)}>
              {ACTION_LABEL[report.target_type] ?? "Застосувати дію"}
            </button>
          )}
          {report.status !== "reviewed" && (
            <button className="btn neutral" disabled={busy} onClick={() => onResolve("reviewed", note)}>Позначити переглянутим</button>
          )}
          {report.status !== "resolved" && (
            <button className="btn ok" disabled={busy} onClick={() => onResolve("resolved", note)}>Закрити скаргу</button>
          )}
        </div>
      </div>
    </div>
  );
}
