"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { myBlocks, unblockUser, errorMessage, type BlockedUserDto } from "../../lib/api";
import { useLang, tr } from "../../lib/i18n";
import { initials } from "../../lib/ui";
import { Ban } from "../components/Icon";

/** «Заблоковані» on the account page (TASK-306): everyone the user blocked, each with an unblock button. */
export default function BlockedList() {
  const { lang } = useLang();
  const [items, setItems] = useState<BlockedUserDto[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    myBlocks().then(setItems).catch((e) => { setError(errorMessage(e)); setItems([]); });
  }, []);

  async function unblock(id: string) {
    setBusy(id);
    setError(null);
    try {
      await unblockUser(id);
      setItems((l) => (l ?? []).filter((x) => x.id !== id));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="card section" id="blocked">
      <h2 className="row gap" style={{ fontSize: 17, marginBottom: 4 }}><Ban className="ico" />{tr(lang, "Заблоковані", "Blocked people")}</h2>
      <p className="muted small" style={{ marginBottom: 12 }}>
        {tr(lang, "Ви не пишете одне одному й не бачите їхніх сповіщень. Вони про це не знають.", "You can't message each other and you don't see their alerts. They are not told.")}
      </p>
      {error && <div className="alert error" role="alert">{error}</div>}
      {items === null ? (
        <div className="skeleton" style={{ height: 48 }} />
      ) : items.length === 0 ? (
        <p className="muted small">{tr(lang, "Ви нікого не блокували.", "You haven't blocked anyone.")}</p>
      ) : (
        <ul className="blocked-list" style={{ listStyle: "none", padding: 0, margin: 0, display: "grid", gap: 8 }}>
          {items.map((u) => (
            <li key={u.id} className="row gap" style={{ alignItems: "center" }}>
              <Link href={`/u/${u.id}`} className="row gap grow" style={{ alignItems: "center", minWidth: 0 }}>
                <span className="avatar sm">{u.avatar_url ? <img src={u.avatar_url} alt="" /> : initials(u.name)}</span>
                <span className="strong" style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{u.name}</span>
              </Link>
              <button className="btn secondary sm" onClick={() => void unblock(u.id)} disabled={busy === u.id}>
                {tr(lang, "Розблокувати", "Unblock")}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
