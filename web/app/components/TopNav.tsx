"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { useLang, tr, tp } from "../../lib/i18n";
import { useUnread } from "../../lib/unread";
import { useSession } from "../../lib/session";
import { initials } from "../../lib/ui";
import { Compass, Ticket, MessageCircle, Plus, User, LayoutGrid, Globe, LogOut, Heart } from "./Icon";
import LanguagePicker from "./LanguagePicker";

/** Desktop top bar (links + create + profile menu) and, under 860 px, the mobile tab bar. */
export default function TopNav() {
  const pathname = usePathname() ?? "/";
  const router = useRouter();
  const { lang } = useLang();
  const { me, logout } = useSession();
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const authed = !!me;
  const unread = useUnread(me?.id, pathname);
  const msgs = unread?.messages ?? 0;
  const alerts = unread?.notifications ?? 0;
  // Chats/Messages carry the unread message count; with none, a plain dot says an alert is waiting (TASK-305).
  const badge = msgs > 0 ? (
    <span className="badge-dot" aria-hidden>{msgs > 99 ? "99+" : msgs}</span>
  ) : alerts > 0 ? <span className="nav-dot" aria-hidden /> : null;
  const badgeText = msgs > 0
    ? tp(lang, msgs, { one: "{n} непрочитане повідомлення", few: "{n} непрочитані повідомлення", many: "{n} непрочитаних повідомлень" }, { one: "{n} unread message", other: "{n} unread messages" })
    : alerts > 0 ? tr(lang, "є нові сповіщення", "new alerts") : "";
  const badgeLabel = badgeText ? <span className="sr-only">{` (${badgeText})`}</span> : null;

  useEffect(() => setOpen(false), [pathname]);
  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => { if (!menuRef.current?.contains(e.target as Node)) setOpen(false); };
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") setOpen(false); };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => { document.removeEventListener("mousedown", onDoc); document.removeEventListener("keydown", onKey); };
  }, [open]);

  const is = (p: string) => (p === "/" ? pathname === "/" || pathname.startsWith("/c/") || pathname.startsWith("/city/") : pathname.startsWith(p));
  // While the session check runs (me === undefined) links go straight to the page; it redirects if needed.
  const gated = (href: string) => (me !== null ? href : `/login?next=${encodeURIComponent(href)}`);
  // A signed-out person who wants to create an event is most likely new: send them to sign-up (it links to login).
  const createHref = me === null ? `/register?next=${encodeURIComponent("/organizer/new")}` : "/organizer/new";

  function signOut() {
    logout();
    router.push("/");
  }

  return (
    <>
      <header className="topbar">
        <div className="topbar-inner">
          <Link href="/" className="wordmark" aria-label={tr(lang, "Underground — на головну", "Underground — home")}>UNDERGROUND</Link>
          <nav className="navlinks" aria-label={tr(lang, "Головне меню", "Main menu")}>
            <Link href="/" className={is("/") ? "active" : ""}>{tr(lang, "Афіша", "Discover")}</Link>
            <Link href={gated("/me/tickets")} className={is("/me/tickets") ? "active" : ""}>{tr(lang, "Квитки", "Tickets")}</Link>
            <Link href={gated("/inbox")} className={is("/inbox") ? "active" : ""}>{tr(lang, "Повідомлення", "Messages")}{badge}{badgeLabel}</Link>
            {authed && <Link href="/organizer" className={is("/organizer") ? "active" : ""}>{tr(lang, "Мої події", "My events")}</Link>}
          </nav>
          <div className="topbar-actions">
            <Link href={createHref} className="btn sm hide-mobile"><Plus className="ico sm" />{tr(lang, "Створити подію", "Create event")}</Link>
            {me === undefined ? (
              <span className="avatar" style={{ background: "var(--surface-2)" }} aria-hidden />
            ) : authed ? (
              <div style={{ position: "relative" }} ref={menuRef}>
                <button className="avatar-btn" aria-expanded={open} aria-controls="profile-menu" aria-label={tr(lang, "Меню профілю", "Profile menu")} onClick={() => setOpen((o) => !o)}>
                  <span className="avatar">{me.avatar_url ? <img src={me.avatar_url} alt="" /> : initials(me.name)}</span>
                </button>
                {open && (
                  // A disclosure panel, not role="menu": it holds a <select> (the language), which a menu may not
                  // contain, and its links work with Tab like any links (fix F12).
                  <div className="menu" id="profile-menu">
                    <div className="menu-head">
                      <div className="strong">{me.name}</div>
                      <div className="muted xs">{me.email}</div>
                    </div>
                    <Link href="/account"><User className="ico sm" />{tr(lang, "Профіль", "Profile")}</Link>
                    <Link href="/me/tickets?tab=saved"><Heart className="ico sm" />{tr(lang, "Збережені", "Saved")}</Link>
                    <Link href="/organizer"><LayoutGrid className="ico sm" />{tr(lang, "Мої події", "My events")}</Link>
                    <div className="menu-lang"><Globe className="ico sm" aria-hidden /><LanguagePicker onPicked={() => setOpen(false)} /></div>
                    <button className="danger" onClick={signOut}><LogOut className="ico sm" />{tr(lang, "Вийти", "Log out")}</button>
                  </div>
                )}
              </div>
            ) : (
              <>
                <LanguagePicker variant="compact" />
                <Link href={`/login?next=${encodeURIComponent(pathname)}`} className="btn ghost sm">{tr(lang, "Увійти", "Log in")}</Link>
              </>
            )}
          </div>
        </div>
      </header>

      <nav className="tabbar" aria-label={tr(lang, "Навігація", "Navigation")}>
        <Link href="/" className={is("/") ? "active" : ""}><Compass className="ico" />{tr(lang, "Афіша", "Discover")}</Link>
        <Link href={gated("/me/tickets")} className={is("/me/tickets") ? "active" : ""}><Ticket className="ico" />{tr(lang, "Квитки", "Tickets")}</Link>
        <Link href={createHref} aria-label={tr(lang, "Створити подію", "Create event")}><span className="create"><Plus className="ico" /></span></Link>
        <Link href={gated("/inbox")} className={is("/inbox") ? "active" : ""}><MessageCircle className="ico" />{tr(lang, "Чати", "Chats")}{badge}{badgeLabel}</Link>
        <Link href={gated("/account")} className={is("/account") || is("/organizer") ? "active" : ""}><User className="ico" />{tr(lang, "Профіль", "Profile")}</Link>
      </nav>
    </>
  );
}
