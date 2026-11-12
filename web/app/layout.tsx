import "./globals.css";
import type { ReactNode } from "react";
import type { Metadata, Viewport } from "next";
import { Onest } from "next/font/google";
import TopNav from "./components/TopNav";
import Footer from "./components/Footer";
import { LangProvider } from "../lib/i18n";
import { SessionProvider } from "../lib/session";

const SITE = process.env.NEXT_PUBLIC_SITE_URL || "http://localhost:3001";

// Onest: a Ukrainian-designed typeface with full Cyrillic. next/font self-hosts it (no request to Google
// from the visitor's browser).
const onest = Onest({ subsets: ["latin", "cyrillic"], variable: "--font-ui", display: "swap" });

export const metadata: Metadata = {
  metadataBase: new URL(SITE),
  title: { default: "Underground — афіша подій і безкоштовні квитки", template: "%s · Underground" },
  description: "Знаходь концерти, вечірки, лекції й зустрічі поруч і бронюй місця безкоштовно. Організатори публікують події за хвилину.",
  openGraph: {
    type: "website",
    siteName: "Underground",
    title: "Underground — афіша подій і безкоштовні квитки",
    description: "Знаходь події поруч і бронюй місця безкоштовно.",
    url: SITE,
  },
  twitter: { card: "summary_large_image" as const },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
  themeColor: "#08090d",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="uk" className={onest.variable}>
      <body>
        <LangProvider>
          <SessionProvider>
            <TopNav />
            {children}
            <Footer />
          </SessionProvider>
        </LangProvider>
      </body>
    </html>
  );
}
