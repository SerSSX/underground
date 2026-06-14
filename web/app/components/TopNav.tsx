"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { getToken, clearToken, getMe } from "../../lib/api";
import { useLang, tr, LanguageSwitcher } from "../../lib/i18n";

/** Public top navigation: afisha for everyone; tickets/cabinet when signed in. */
export default function TopNav() {
  const pathname = usePathname();
  const router = useRouter();
  const [name, setName] = useState<string | null>(null);
  const [authed, setAuthed] = useState(false);
  const { lang } = useLang();

  useEffect(() => {
    if (!getToken()) {
      setAuthed(false);
      setName(null);
      return;
    }
    setAuthed(true);
    getMe()
      .then((u) => setName(u.name))
      .catch(() => {
        clearToken();
        setAuthed(false);
      });
  }, [pathname]);

  function logout() {
    clearToken();
    setAuthed(false);
    router.push("/");
  }

  const link = (href: string, label: string) => (
    <Link href={href} className={pathname === href ? "active" : ""}>{label}</Link>
  );

  return (
    <nav className="nav">
      <Link href="/" className="wordmark" style={{ color: "var(--text)" }}>UNDERGROUND</Link>
      {link("/", tr(lang, "Афіша", "Discover"))}
      {authed && link("/me/tickets", tr(lang, "Мої квитки", "My tickets"))}
      {authed && link("/organizer", tr(lang, "Кабінет", "Cabinet"))}
      {authed && link("/account", tr(lang, "Акаунт", "Account"))}
      <span className="spacer" />
      <LanguageSwitcher />
      {authed ? (
        <>
          {name && <span className="muted">{name}</span>}
          <button className="btn ghost sm" onClick={logout}>{tr(lang, "Вийти", "Log out")}</button>
        </>
      ) : (
        <Link href="/login" className="btn sm" style={{ color: "#fff" }}>{tr(lang, "Увійти", "Log in")}</Link>
      )}
    </nav>
  );
}
