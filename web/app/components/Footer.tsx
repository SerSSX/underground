"use client";

import Link from "next/link";
import { useLang, tr } from "../../lib/i18n";
import AppBanner from "./AppBanner";

export default function Footer() {
  const { lang } = useLang();
  return (
    <footer className="footer">
      <div className="footer-inner">
        <AppBanner />
        <div className="row spread wrap" style={{ gap: 16 }}>
          <span className="wordmark" style={{ fontSize: 13 }}>UNDERGROUND</span>
          <nav className="footer-links" aria-label={tr(lang, "Інформація", "Information")}>
            <Link href="/terms">{tr(lang, "Умови", "Terms")}</Link>
            <Link href="/privacy">{tr(lang, "Конфіденційність", "Privacy")}</Link>
            <Link href="/abuse">{tr(lang, "Поскаржитися", "Report abuse")}</Link>
            <Link href="/download">{tr(lang, "Застосунок", "App")}</Link>
          </nav>
        </div>
        {/* Place names and addresses come from OpenStreetMap (ODbL, accepted by the owner 2026-11-13). */}
        <p className="subtle xs footer-attrib">
          <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noreferrer">{tr(lang, "© учасники OpenStreetMap", "© OpenStreetMap contributors")}</a>
        </p>
      </div>
    </footer>
  );
}
