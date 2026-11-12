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
            <Link href="/app">{tr(lang, "Застосунок", "App")}</Link>
          </nav>
        </div>
      </div>
    </footer>
  );
}
