"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, useState, type ReactNode } from "react";
import { getToken, clearToken, getMe, errorMessage } from "../../lib/api";

const LINKS = [
  { href: "/dashboard", label: "Дашборд", icon: "▦" },
  { href: "/users", label: "Користувачі", icon: "👥" },
  { href: "/events", label: "Події", icon: "🎟" },
  { href: "/orders", label: "Замовлення", icon: "🧾" },
  { href: "/reports", label: "Скарги", icon: "⚑" },
  { href: "/actions", label: "Журнал дій", icon: "🗒" },
  { href: "/payouts", label: "Виплати", icon: "💸" },
  { href: "/telemetry", label: "Телеметрія", icon: "📈" },
  { href: "/payments", label: "Платіжки", icon: "⚙" },
];

/** Common chrome: auth guard + persistent sidebar nav + logout. Wraps every console page. */
export default function Shell({ title, subtitle, children }: { title: string; subtitle?: string; children: ReactNode }) {
  const router = useRouter();
  const pathname = usePathname();
  const [me, setMe] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!getToken()) {
      router.push("/login");
      return;
    }
    getMe()
      .then((u) => {
        if (u.role !== "ADMIN") {
          clearToken();
          router.push("/login");
          return;
        }
        setMe(u.name);
      })
      .catch((e) => {
        if (errorMessage(e).includes("Сесія")) {
          clearToken();
          router.push("/login");
        }
      });
  }, [router]);

  // Close the mobile drawer on route change.
  useEffect(() => setOpen(false), [pathname]);

  function logout() {
    clearToken();
    router.push("/login");
  }

  return (
    <div className="app">
      {open && <div className="scrim" onClick={() => setOpen(false)} />}
      <aside className={open ? "sidebar open" : "sidebar"}>
        <div className="sidebar-brand">UNDERGROUND<small>Адмін-панель</small></div>
        <nav className="sidebar-nav">
          {LINKS.map((l) => (
            <Link key={l.href} href={l.href} className={pathname.startsWith(l.href) ? "snav active" : "snav"}>
              <span className="ico" aria-hidden>{l.icon}</span>
              {l.label}
            </Link>
          ))}
        </nav>
        <div className="sidebar-foot">
          {me && <span className="who">{me}</span>}
          <button className="btn ghost sm" onClick={logout}>Вийти</button>
        </div>
      </aside>

      <main className="content">
        <div className="topbar">
          <button className="btn ghost sm menu-btn" onClick={() => setOpen(true)} aria-label="Меню">☰</button>
          <div>
            <h1 className="page-title">{title}</h1>
          </div>
        </div>
        {subtitle && <p className="page-sub">{subtitle}</p>}
        {children}
      </main>
    </div>
  );
}
