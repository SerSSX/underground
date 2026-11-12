"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, useRef, useState } from "react";
import { useLang, tr } from "../../lib/i18n";
import { useSession } from "../../lib/session";
import { initials } from "../../lib/ui";
import { Compass, Ticket, MessageCircle, Plus, User, LayoutGrid, Globe, LogOut } from "./Icon";

/** Desktop top bar (links + create + profile menu) and, under 860 px, the mobile tab bar. */
export default function TopNav() {
  const pathname = usePathname() ?? "/";
  const router = useRouter();
  const { lang, setLang } = useLang();
  const { me, logout } = useSession();
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const authed = !!me;

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
  const createHref = gated("/organizer/new");

  function signOut() {
    logout();
    router.push("/");
  }

  return (
    <>
      <header className="topbar">
        <div className="topbar-inner">
          <Link href="/" className="wordmark" aria-label="Underground — на головну">UNDERGROUND</Link>
          <nav className="navlinks" aria-label={tr(lang, "Головне меню", "Main menu")}>
            <Link href="/" className={is("/") ? "active" : ""}>{tr(lang, "Афіша", "Discover")}</Link>
            <Link href={gated("/me/tickets")} className={is("/me/tickets") ? "active" : ""}>{tr(lang, "Квитки", "Tickets")}</Link>
            <Link href={gated("/inbox")} className={is("/inbox") ? "active" : ""}>{tr(lang, "Повідомлення", "Messages")}</Link>
            {authed && <Link href="/organizer" className={is("/organizer") ? "active" : ""}>{tr(lang, "Мої події", "My events")}</Link>}
          </nav>
          <div className="topbar-actions">
            <Link href={createHref} className="btn sm hide-mobile"><Plus className="ico sm" />{tr(lang, "Створити подію", "Create event")}</Link>
            {me === undefined ? (
              <span className="avatar" style={{ background: "var(--surface-2)" }} aria-hidden />
            ) : authed ? (
              <div style={{ position: "relative" }} ref={menuRef}>
                <button className="avatar-btn" aria-haspopup="menu" aria-expanded={open} aria-label={tr(lang, "Меню профілю", "Profile menu")} onClick={() => setOpen((o) => !o)}>
                  <span className="avatar">{me.avatar_url ? <img src={me.avatar_url} alt="" /> : initials(me.name)}</span>
                </button>
                {open && (
                  <div className="menu" role="menu">
                    <div className="menu-head">
                      <div className="strong">{me.name}</div>
                      <div className="muted xs">{me.email}</div>
                    </div>
                    <Link href="/account" role="menuitem"><User className="ico sm" />{tr(lang, "Профіль", "Profile")}</Link>
                    <Link href="/organizer" role="menuitem"><LayoutGrid className="ico sm" />{tr(lang, "Мої події", "My events")}</Link>
                    <button role="menuitem" onClick={() => setLang(lang === "uk" ? "en" : "uk")}><Globe className="ico sm" />{lang === "uk" ? "English" : "Українська"}</button>
                    <button role="menuitem" className="danger" onClick={signOut}><LogOut className="ico sm" />{tr(lang, "Вийти", "Log out")}</button>
                  </div>
                )}
              </div>
            ) : (
              <>
                <button className="btn quiet sm hide-mobile" onClick={() => setLang(lang === "uk" ? "en" : "uk")} aria-label="Language / Мова">{lang === "uk" ? "EN" : "UA"}</button>
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
        <Link href={gated("/inbox")} className={is("/inbox") ? "active" : ""}><MessageCircle className="ico" />{tr(lang, "Чати", "Chats")}</Link>
        <Link href={gated("/account")} className={is("/account") || is("/organizer") ? "active" : ""}><User className="ico" />{tr(lang, "Профіль", "Profile")}</Link>
      </nav>
    </>
  );
}
