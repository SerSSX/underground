import "./globals.css";
import type { ReactNode } from "react";

export const metadata = {
  title: "Underground Admin",
  description: "Underground admin panel",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="uk">
      <body>{children}</body>
    </html>
  );
}
