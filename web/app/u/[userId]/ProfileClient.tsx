"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import {
  getProfile, userEvents, followUser, unfollowUser, getToken, isOver, isUuid, errorMessage, ApiError,
  type EventDto, type PublicProfile,
} from "../../../lib/api";
import { useLang, tr, tp } from "../../../lib/i18n";
import { useSession } from "../../../lib/session";
import { initials } from "../../../lib/ui";
import EventCard from "../../components/EventCard";
import { CircleCheck, Compass, MessageCircle, Pencil, UserPlus, Check } from "../../components/Icon";

/** True for "this person does not exist (or is blocked)": the API's 404, or a malformed id it refuses. */
function isMissing(e: unknown): boolean {
  return e instanceof ApiError && (e.status === 404 || e.status === 400);
}

/**
 * A person's public page (TASK-305): name, photo, bio, counters, their upcoming published events and, folded,
 * the past ones. Signed-in visitors can write to them or follow them; nobody is offered to write to themselves.
 */
export default function ProfileClient({ userId, initialProfile }: { userId: string; initialProfile: PublicProfile | null }) {
  const router = useRouter();
  const { lang } = useLang();
  const { me } = useSession();
  const [profile, setProfile] = useState<PublicProfile | null>(initialProfile);
  const [missing, setMissing] = useState(!isUuid(userId));
  const [events, setEvents] = useState<EventDto[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isUuid(userId)) { setMissing(true); return; }
    let stop = false;
    // Read again in the browser: signed in, the API also says whether the visitor follows this person.
    getProfile(userId)
      .then((p) => { if (!stop) { setProfile(p); setMissing(false); } })
      .catch((e) => { if (stop) return; if (isMissing(e)) setMissing(true); else setError(errorMessage(e)); });
    userEvents(userId)
      .then((l) => { if (!stop) setEvents(l); })
      .catch(() => { if (!stop) setEvents([]); });
    return () => { stop = true; };
  }, [userId, me?.id]);

  if (missing) {
    return (
      <main className="container narrow">
        <div className="empty" style={{ marginTop: 24 }}>
          <div className="ei"><Compass className="ico lg" /></div>
          <h1 style={{ fontSize: 20, marginBottom: 6 }}>{tr(lang, "Такого профілю немає", "Profile not found")}</h1>
          <p>{tr(lang, "Можливо, посилання застаріло або акаунт видалено.", "The link may be out of date, or the account was deleted.")}</p>
          <Link className="btn" href="/">{tr(lang, "До афіші", "Browse events")}</Link>
        </div>
      </main>
    );
  }
  if (!profile) {
    return (
      <main className="container narrow">
        {error ? <div className="alert error" style={{ marginTop: 24 }}>{error}</div> : <div className="skeleton" style={{ height: 200, marginTop: 24 }} />}
      </main>
    );
  }

  const self = !!me && me.id === profile.id;
  // /inbox sends a signed-out visitor to sign-in and back here with the same `to`, so the link is the same for all.
  const chatHref = `/inbox?to=${profile.id}`;
  const upcoming = (events ?? []).filter((e) => !isOver(e.starts_at, e.ends_at));
  const past = (events ?? []).filter((e) => isOver(e.starts_at, e.ends_at)).reverse();

  async function toggleFollow() {
    if (!profile) return;
    if (!getToken()) { router.push(`/login?next=${encodeURIComponent(`/u/${profile.id}`)}`); return; }
    const was = !!profile.is_following;
    setBusy(true);
    setError(null);
    try {
      if (was) await unfollowUser(profile.id);
      else await followUser(profile.id);
      const delta = was ? -1 : 1;
      setProfile({ ...profile, is_following: !was, followers_count: Math.max(0, (profile.followers_count ?? 0) + delta) });
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="container">
      <section className="card profile-head">
        <span className="avatar lg profile-avatar">{profile.avatar_url ? <img src={profile.avatar_url} alt="" /> : initials(profile.name)}</span>
        <div className="grow" style={{ minWidth: 0 }}>
          <h1 className="profile-name">
            {profile.name}
            {profile.verified && <CircleCheck className="ico sm" style={{ color: "var(--accent-fg)", marginLeft: 8, verticalAlign: -2 }} aria-label={tr(lang, "перевірений", "verified")} />}
          </h1>
          {profile.is_organizer && <div className="muted small">{tr(lang, "Організатор", "Organiser")}</div>}
          <div className="row gap wrap muted small" style={{ marginTop: 6 }}>
            <span>{tp(lang, profile.events_count, { one: "{n} подія", few: "{n} події", many: "{n} подій" }, { one: "{n} event", other: "{n} events" })}</span>
            {profile.followers_count != null && (
              <span>{tp(lang, profile.followers_count, { one: "{n} підписник", few: "{n} підписники", many: "{n} підписників" }, { one: "{n} follower", other: "{n} followers" })}</span>
            )}
            {profile.rating_avg != null && profile.rating_count > 0 && (
              <span>★ {profile.rating_avg.toFixed(1)} · {tp(lang, profile.rating_count, { one: "{n} відгук", few: "{n} відгуки", many: "{n} відгуків" }, { one: "{n} review", other: "{n} reviews" })}</span>
            )}
          </div>
          {profile.bio && <p className="profile-bio">{profile.bio}</p>}
          <div className="row gap wrap" style={{ marginTop: 14 }}>
            {self ? (
              <>
                <span className="badge">{tr(lang, "Це ваш профіль", "This is your profile")}</span>
                <Link className="btn secondary sm" href="/account"><Pencil className="ico sm" aria-hidden />{tr(lang, "Редагувати", "Edit")}</Link>
              </>
            ) : (
              <>
                <Link className="btn sm" href={chatHref}>
                  <MessageCircle className="ico sm" aria-hidden />{tr(lang, "Написати", "Message")}
                </Link>
                <button className={`btn sm ${profile.is_following ? "secondary" : "ghost"}`} onClick={() => void toggleFollow()} disabled={busy} aria-pressed={!!profile.is_following}>
                  {profile.is_following ? <Check className="ico sm" aria-hidden /> : <UserPlus className="ico sm" aria-hidden />}
                  {profile.is_following ? tr(lang, "Ви підписані", "Following") : tr(lang, "Підписатися", "Follow")}
                </button>
              </>
            )}
          </div>
        </div>
      </section>
      {error && <div className="alert error" style={{ marginTop: 12 }}>{error}</div>}

      <section className="section">
        <div className="section-head"><h2>{tr(lang, "Найближчі події", "Upcoming events")}</h2></div>
        {events === null ? (
          <div className="skeleton" style={{ height: 220 }} />
        ) : upcoming.length === 0 ? (
          <p className="muted">{tr(lang, "Найближчих подій немає.", "No upcoming events.")}</p>
        ) : (
          <div className="grid">{upcoming.map((e, i) => <EventCard key={e.id} event={e} eager={i < 4} />)}</div>
        )}
      </section>

      {past.length > 0 && (
        <details className="section past-events">
          <summary className="strong">{tp(lang, past.length, { one: "Минула подія: {n}", few: "Минулі події: {n}", many: "Минулі події: {n}" }, { one: "Past event: {n}", other: "Past events: {n}" })}</summary>
          <div className="grid" style={{ marginTop: 14 }}>{past.map((e) => <EventCard key={e.id} event={e} />)}</div>
        </details>
      )}
    </main>
  );
}
