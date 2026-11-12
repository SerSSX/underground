import type { ReactNode } from "react";

/** Centred card used by sign-in, sign-up and password pages. */
export default function AuthCard({ title, subtitle, children, footer }: { title: string; subtitle?: string; children: ReactNode; footer?: ReactNode }) {
  return (
    <main className="container tight">
      <div className="card" style={{ marginTop: 24, padding: "32px 28px", borderRadius: "var(--r-xl)" }}>
        <div className="wordmark" style={{ fontSize: 12, color: "var(--muted)", marginBottom: 18 }}>UNDERGROUND</div>
        <h1 style={{ fontSize: 28 }}>{title}</h1>
        {subtitle && <p className="muted" style={{ marginTop: 8 }}>{subtitle}</p>}
        <div style={{ marginTop: 24 }}>{children}</div>
      </div>
      {footer && <div style={{ textAlign: "center", marginTop: 18 }} className="muted">{footer}</div>}
    </main>
  );
}
