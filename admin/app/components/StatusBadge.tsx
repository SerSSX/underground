// Semantic status badge — one total mapper over every real status in the platform.
// No catch-all-to-red: unknown/none render neutral gray. A leading glyph keeps it
// colorblind-safe. Used for order, escrow, KYC, event, payout and report columns.

type Tone = "ok" | "warn" | "danger" | "neutral" | "accent";

const MAP: Record<string, { tone: Tone; glyph: string; label?: string }> = {
  // order status
  paid: { tone: "ok", glyph: "✓" },
  pending: { tone: "warn", glyph: "◷" },
  refunded: { tone: "danger", glyph: "↩" },
  // escrow status
  held: { tone: "warn", glyph: "◷" },
  clearing: { tone: "warn", glyph: "◷" },
  cleared: { tone: "ok", glyph: "✓" },
  paid_out: { tone: "ok", glyph: "✓" },
  frozen: { tone: "danger", glyph: "✕" },
  none: { tone: "neutral", glyph: "–" },
  // KYC
  verified: { tone: "ok", glyph: "✓" },
  rejected: { tone: "danger", glyph: "✕" },
  // event moderation
  published: { tone: "ok", glyph: "✓" },
  hidden: { tone: "neutral", glyph: "◐" },
  blocked: { tone: "danger", glyph: "✕" },
  draft: { tone: "neutral", glyph: "✎" },
  // report status
  open: { tone: "warn", glyph: "◷" },
  reviewed: { tone: "accent", glyph: "◉" },
  resolved: { tone: "ok", glyph: "✓" },
  // generic
  active: { tone: "ok", glyph: "✓" },
  failed: { tone: "danger", glyph: "✕" },
  processing: { tone: "warn", glyph: "◷" },
  succeeded: { tone: "ok", glyph: "✓" },
};

export default function StatusBadge({ status }: { status: string | null | undefined }) {
  const key = (status ?? "none").toLowerCase();
  const meta = MAP[key] ?? { tone: "neutral" as Tone, glyph: "•" };
  return (
    <span className={`badge2 s-${meta.tone}`}>
      <span className="g" aria-hidden>{meta.glyph}</span>
      {meta.label ?? status ?? "—"}
    </span>
  );
}
