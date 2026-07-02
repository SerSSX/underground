"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { myNotifications, markNotificationsRead, getToken, errorMessage, type NotificationDto } from "../../lib/api";
import { useLang, tr, fmtDateL } from "../../lib/i18n";

export default function InboxPage() {
  const router = useRouter();
  const { lang } = useLang();
  const [items, setItems] = useState<NotificationDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!getToken()) {
      router.push("/login?next=/inbox");
      return;
    }
    myNotifications()
      .then((list) => {
        setItems(list);
        if (list.some((n) => !n.read)) void markNotificationsRead().catch(() => {});
      })
      .catch((e) => setError(errorMessage(e)))
      .finally(() => setLoading(false));
  }, [router]);

  return (
    <main className="container narrow">
      <h1 style={{ fontSize: 24, margin: "8px 0 14px" }}>{tr(lang, "Сповіщення", "Notifications")}</h1>
      {error && <div className="error">{error}</div>}
      {loading && <p className="muted">{tr(lang, "Завантаження…", "Loading…")}</p>}
      {!loading && items.length === 0 && (
        <p className="muted">{tr(lang, "Поки тихо. Обери інтереси в профілі — і ми підкажемо нові події.", "All quiet. Pick interests in your profile and we'll surface new events.")}</p>
      )}
      <div className="grid">
        {items.map((n) => {
          const clickable = !!n.event_id;
          return (
            <div
              key={n.id}
              className="ticket"
              role={clickable ? "button" : undefined}
              tabIndex={clickable ? 0 : undefined}
              onClick={clickable ? () => router.push(`/events/${n.event_id}`) : undefined}
              onKeyDown={clickable ? (e) => { if (e.key === "Enter") router.push(`/events/${n.event_id}`); } : undefined}
              style={{ cursor: clickable ? "pointer" : "default" }}
            >
              <strong style={{ color: n.read ? undefined : "var(--accent)" }}>{n.title}</strong>
              {n.body && <div className="muted" style={{ fontSize: 14, marginTop: 2 }}>{n.body}</div>}
              <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>{fmtDateL(n.created_at, lang)}</div>
            </div>
          );
        })}
      </div>
    </main>
  );
}
