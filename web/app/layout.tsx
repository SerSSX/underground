import "./globals.css";
import type { ReactNode } from "react";
import type { Metadata, Viewport } from "next";
import { Onest } from "next/font/google";
import TopNav from "./components/TopNav";
import Footer from "./components/Footer";
import NoZoom from "./components/NoZoom";
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

// Owner's request (TASK-304): on phones the page keeps its size, no pinch or double-tap zoom. Known cost: people
// with low vision cannot zoom in (WCAG 1.4.4); iOS Safari ignores user-scalable=no and still allows pinch, so
// <NoZoom /> blocks the pinch by script (TASK-305). Fields are 16 px on phones (globals.css), so iOS no longer zooms
// in when one is focused.
export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  maximumScale: 1,
  userScalable: false,
  viewportFit: "cover",
  themeColor: "#08090d",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="uk" className={onest.variable}>
      <body>
        <LangProvider>
          <SessionProvider>
            <NoZoom />
            <TopNav />
            {children}
            <Footer />
          </SessionProvider>
        </LangProvider>
      </body>
    </html>
  );
}
