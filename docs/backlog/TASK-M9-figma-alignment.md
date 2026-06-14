# M9 — Figma design alignment (BLOCKED on Figma MCP / fresh session)

**Status: blocked.** The Figma Dev Mode MCP (`figma-desktop`, http://127.0.0.1:3845/mcp) loads
**only in a fresh session** — its tools are not present in the session that built M1–M8, so the
44 design frames cannot be read here. Doing visual "alignment" without the reference frames would
be guessing and could conflict with the real design, so M9 is deferred to a fresh session.

This document is the ready-to-execute plan for that session.

## Prerequisites (fresh session)
1. Start a brand-new Claude Code session so the `figma-desktop` MCP tools load.
2. Confirm the Figma desktop app is open on the "underground" file (id `hZvaYaoCKfr1XyqqkdAHJ4`).
3. Pull each frame via `mcp__figma*` `get_code` / `get_image` by node-id (URL form `A-B` → API form `A:B`).

## The 44 frame node-ids (API form)
139-3, 116-969, 308-1800, 141-1035, 116-818, 136-257, 308-1695, 131-265, 144-364, 233-788,
323-1461, 194-377, 308-1133, 1-2, 141-430, 91-249, 87-352, 313-1131, 57-77, 87-499, 18-18, 9-23,
91-204, 45-47, 91-108, 34-211, 308-1336, 200-965, 92-412, 141-591, 106-443, 233-1081, 308-1570,
141-872, 200-1098, 308-1234, 106-674, 92-251, 92-137, 272-1334, 333-1243, 323-1166, 354-234, 354-0.

## Execution plan
1. **Extract design tokens** from the frames: color palette, typography scale, spacing, radii,
   elevation. Reconcile against the current dark-theme tokens (Android `ui/theme/`, web `app/globals.css`,
   admin styles). Produce a single source-of-truth token set.
2. **Per-surface frame → screen mapping.** Group the 44 frames by screen (afisha, event detail,
   ticket/QR, onboarding, profile/wall, chat, organizer create/edit, auth). For each, diff the Figma
   layout against the built screen and list concrete deltas (spacing, type, component variants, states).
3. **Component pass.** Align shared components (event card, buttons, chips, bottom nav, inputs) to the
   Figma component set; codify variants. Android Compose theme + web/admin CSS.
4. **Polish** empty/loading/error states and motion to match the frames.
5. **Verify**: Android `assembleDebug` + device screenshots vs frames; web/admin `tsc` + `next build`
   + screenshot comparison. No backend changes expected (M9 is presentation-only).

## Notes
- The design system foundation already exists (TASK-014 established dark theme + components); M9 is a
  reconciliation/polish pass against the canonical Figma, not a from-scratch build.
- Keep changes presentation-only — the API contracts (M1–M8) are stable and 100%-covered; don't
  perturb them for visual tweaks.
