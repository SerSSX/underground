import "./globals.css";
import type { ReactNode } from "react";
import TopNav from "./components/TopNav";
import { LangProvider } from "../lib/i18n";

const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

export const metadata = {
  metadataBase: new URL(SITE),
  title: { default: "UNDERGROUND — events", template: "%s" },
  description: "Underground events worldwide: concerts, parties, happenings. Discover and get tickets.",
  openGraph: {
    type: "website",
    siteName: "Underground",
    title: "UNDERGROUND — events",
    description: "Underground events worldwide: concerts, parties, happenings. Discover and get tickets.",
    url: SITE,
  },
  twitter: { card: "summary_large_image" as const },
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="uk">
      <body>
        <LangProvider>
          <TopNav />
          {children}
          <footer className="container muted" style={{ marginTop: 48, paddingTop: 16, borderTop: "1px solid var(--border)", fontSize: 13 }}>
            <a href="/privacy">Конфіденційність</a> · <a href="/terms">Умови</a> ·{" "}
            <a href="/refunds">Повернення</a> · <a href="/abuse">Скарги</a> ·{" "}
            <a href="/account">Акаунт</a>
          </footer>
        </LangProvider>
      </body>
    </html>
  );
}
