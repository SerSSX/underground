# UX + Product-Design audit v2 — 2026-06-14

Multi-agent audit (6 agents: web / admin / Android / competitive-benchmark / full-stack-scenarios →
synthesis) **grounded in live browser screenshots** driven this session (real login into the admin
+ public web walkthrough). Benchmarked against Dice, Resident Advisor, Luma, Eventbrite (consumer)
and Stripe / Linear / Vercel / Retool (admin).

## Scores

| Surface | Score |
|---|---|
| Public web client | **72** |
| Admin console | **28** ← the liability |
| Android app | **70** |
| Backend | 68 |
| Competitive readiness | 46 |
| Full-stack E2E flow | 56 |
| **Overall** | **54** |

## Verdict

A credible MVP with a **genuinely good public web client** and a **decent Android app** on a solid
backend — but the **admin console is the headline liability**: it is structurally broken for its one
job, moderation. (Note: the "escrow funds hang forever / no worker" claim was **investigated and is
false** — `escrow_worker` + `escrow::sweep` auto-advance held→clearing→cleared; downgraded.)

The "white admin screen" you saw was **a stale dev tab** (recompiled chunks 404'd) — not a code bug;
hard-refresh fixes it. The real issue is that the admin, even when it loads, looks and behaves unfinished.

## Top problems (ordered)

1. **Admin tables clip the actions column off-screen** — `<table className="table">` has no overflow
   wrapper and the page is capped in a ~1080px card, so Refund/Freeze/Release, user-block and KYC
   buttons are **unreachable below ~1400px**. The core moderation/escrow workflow is literally
   non-functional on a normal laptop. *(verified: admin/app/orders/page.tsx, globals.css)*
2. **Admin status badges are semantically blank** — `pill()` maps held/frozen→amber,
   cleared/paid_out→green, **everything else→red**, so paid / pending / refunded / none are
   indistinguishable and "no escrow" looks like an error. At-a-glance scanning (the admin's whole
   point) is impossible. *(verified: admin/app/orders/page.tsx:30)*
3. **Admin shell wastes the viewport & looks unfinished** — nav is a wrapping row of text links
   inside the content card; titles wrap to 2 lines and shove the user/logout cluster; no sidebar,
   no persistent chrome, no responsive layout.
4. **Admin dashboard answers nothing** — 5 flat number+label boxes, ~60% empty viewport, no
   trend/alerts (pending disputes, frozen escrow, failed payouts). Can't read platform health in <2s.
5. **High-risk escrow actions have no safe affordance** — force-release of a disputed order fires
   from a bare `confirm()` with no order context. One misclick moves real money irreversibly.
6. **Buyer-side money/trust confirmation gaps** — paid checkout redirects with no order #/receipt;
   disputes show a spinner with no timeline; escrow explained in a buried 13px paragraph.
7. **Age-gating incomplete for unauthenticated users** — gate is text-only; an unauthed minor can
   proceed toward payment and fail late. Compliance/liability risk (web + Android agree).
8. **Paid-ticket transfer leaks financial state** — `transfer_ticket` reassigns owner but doesn't
   move/guard the order/escrow; the original buyer keeps money-at-risk for a ticket they gave away.

## Design direction

**ADMIN is the headline — rebuild the shell + 4 primitives, in order:**

1. **App-shell** — replace the single 1080px card with a persistent 2-column layout: fixed left
   sidebar (240–260px: wordmark, vertical icon+label nav with active pill at `--accent`@~13%,
   user/logout pinned to footer); main content full-bleed & fluid; page title becomes an in-content
   H1; mobile → hamburger drawer. This alone kills the wrapping nav, the clipped tables (full width
   restores the actions column on most displays), and the "unfinished demo" feel.
2. **`<DataTable>` primitive** (orders/users/events/reports/payouts/actions) — `overflow-x:auto`
   wrapper **+ sticky right-hand actions column** (`position:sticky; right:0`); sortable headers;
   pagination; first-class skeleton-loading / empty / error-with-Retry states; density toggle.
3. **Status-badge system** — one **total** mapper, no catch-all-to-red: green = paid/cleared/paid_out/
   approved/active; amber = pending/held/clearing; red = frozen/refunded/rejected/blocked/failed;
   gray = none/draft/unknown. Leading glyph for colorblind safety. Applied to every status column.
4. **Stat-card system** — icon + value + 7-day trend chip + inline sparkline, 2–3 per row, plus a
   triage alert strip (pending disputes / frozen escrow / failed payouts / KYC-awaiting).
5. **Escrow action drawer** — replaces inline buttons + `confirm()`: state badge + read-only order
   summary; Refund=danger, Release=primary, Freeze=neutral; explicit "force" checkbox + "cannot be
   undone" copy. Pattern reused for user-block / event-moderate / report-resolve.

**Shared design layer:** YES for **tokens** (web & admin already duplicate `--accent/--bg/--surface`
and are drifting; extract one CSS-variable file + mirror as Kotlin constants). **NO** to a full
cross-app component library this quarter.

**WEB (already decent):** only cheap, high-trust wins — missing `.savebtn`/`.reason` CSS,
keyboard-navigable cards, real checkout receipt. No redesign.

**ANDROID:** parity-driven — adopt the shared status palette+glyphs, rich empty/error+Retry,
prominent age-gate banner, contentDescription pass. No rebuild.

---


## Tasks (full)
### ADM-1 — Rebuild admin app-shell: persistent sidebar + full-width content  [P0 · L · admin]
**Rationale:** Shell.tsx wraps every page in a 1080px card with a wrapping text-link nav; this is the root cause of the clipped tables, wrapping titles, and 'unfinished' feel. Fixing the shell unblocks ADM-2 and immediately restores most of the clipped actions column.
**Acceptance:** Left sidebar (icons+labels, active state, user/logout footer) on >=1024px; content is full-width with no card cap; page title is an H1 in content, not header; nav never wraps; mobile shows a hamburger drawer.

### ADM-2 — Build reusable <DataTable>: scroll-contain + sticky actions + states  [P0 · L · admin]
**Rationale:** Tables have no overflow wrapper, so the 'Дії' actions column clips off-screen and moderation/escrow buttons are unreachable — the core admin job is broken. Centralizing also fixes empty/loading/error inconsistency across 6 pages.
**Acceptance:** Actions column always visible via sticky-right or guaranteed scroll-contained wrapper; sortable headers; pagination/infinite scroll; skeleton loading, icon empty state, and error state with Retry; used by /orders, /users, /events, /reports, /payouts, /actions.

### ADM-3 — Semantic status-badge mapper with total state coverage + glyphs  [P0 · S · admin]
**Rationale:** pill() in orders/page.tsx collapses all statuses into amber/green/red with a catch-all-to-red, so paid/pending/refunded/none are indistinguishable and 'none' looks like an error. Tiny code change, outsized scanning impact.
**Acceptance:** Single mapper covers every real status (paid/cleared/paid_out/approved=green; pending/held/clearing=amber; frozen/refunded/rejected/blocked=red; none/draft=gray) with a leading glyph; applied to escrow, order, KYC, event, payout, report columns; 'none' is gray not red.

### ADM-4 — Escrow action drawer replacing inline buttons + confirm()  [P0 · M · admin]
**Rationale:** Force-release of a disputed order fires from a bare confirm() with no order context — highest-risk money operation has the weakest affordance. One misclick moves real funds irreversibly.
**Acceptance:** Drawer shows state badge + read-only order summary (amount, buyer, event); Refund=danger, Release=primary, Freeze=neutral; explicit 'force' checkbox with explanation; irreversible actions show 'cannot be undone'; no raw confirm() in the escrow path.

### ADM-5 — Dashboard stat-cards (icon+trend+sparkline) + triage alert strip  [P1 · M · admin]
**Rationale:** Dashboard is 5 flat boxes with 60% empty viewport and no health signal; admins can't see pending disputes/frozen escrow/failed payouts at a glance.
**Acceptance:** Each stat has icon, value, 7d trend chip, inline sparkline; 2-3 per row; an alerts strip links to pending disputes, frozen-escrow orders, failed payouts, KYC-awaiting-review.

### FS-1 — Extract shared design tokens (CSS vars) for web + admin  [P1 · S · cross]
**Rationale:** web and admin globals.css duplicate identical tokens (--accent/--bg/--surface) and are already drifting (web --surface2, incomplete admin badge palette). A single token file stops drift and makes the status palette authoritative; mirror as Kotlin constants for Android.
**Acceptance:** One shared CSS variable file (colors, status palette, spacing 4/8/12/16/24, radii) imported by both Next apps; Android Theme.kt mirrors the same values; no divergent hardcoded duplicates of these tokens remain.

### ADM-6 — Report-triage detail view: reporter, similar reports, decision notes  [P1 · M · admin]
**Rationale:** Reports can be resolved but there's no triage context (reporter, pattern of similar reports on same target, decision notes) — weak for a trust-and-safety queue.
**Acceptance:** Report detail shows reporter + date + category, target with link, similar open reports on the same target, and a required decision-notes field captured into the action log before resolving.

### ADM-7 — Payouts ops: status filter, failure reason, retry  [P2 · M · admin]
**Rationale:** Payouts page is a bare list with no status workflow or failure visibility; admins can't see/retry failed payouts.
**Acceptance:** Filter by status (pending/processing/succeeded/failed); each row shows organizer, amount, bank last-4, status, failure reason; failed payouts have a Retry action.

### ADM-8 — Action-log detail: resource links + before/after change  [P2 · S · admin]
**Rationale:** Action log shows only action+timestamp+actor with no context or click-through; weak for compliance/accountability.
**Acceptance:** Each entry shows clickable resource (user/event/report) and a change summary (e.g. role USER→BLOCKED); filterable by action type, actor, date range.

### WEB-1 — Add missing .savebtn and .reason CSS + keyboard-navigable event card  [P1 · S · web]
**Rationale:** EventCard.tsx renders className 'savebtn' and 'reason' but neither is in globals.css (verified), so elements are unstyled; the card is a div.ecard with onClick, not keyboard-navigable. Cheap, high-trust polish on the otherwise-good web surface.
**Acceptance:** savebtn and reason have styles; save button has an aria-label; the card is reachable and activatable via keyboard (Link or button), focus-visible state present.

### WEB-2 — Checkout receipt: order #, date, email-sent confirmation, escrow timeline  [P1 · M · web]
**Rationale:** Paid checkout redirects to /me/tickets with no order number, receipt, or confirmation message — buyers lack proof of purchase, driving support load and trust anxiety on the revenue flow.
**Acceptance:** After successful purchase a receipt is shown/linked with order #, date, total, 'confirmation email sent to X', and a visual escrow timeline; receipt reachable from ticket detail.

### WEB-3 — Dispute status timeline + resolution feedback  [P1 · M · web]
**Rationale:** Disputes show a spinner pill with no expected timeline and no notification/state update on resolution; high-anxiety, high-stakes flow with no feedback.
**Acceptance:** Dispute view shows opened-at + expected review window; on escrow status change the buyer is notified and the ticket/escrow badge updates (poll or webhook) without manual refresh.

### WEB-4 — Escrow explanation surfaced (highlighted box + per-state hints)  [P2 · S · web]
**Rationale:** Escrow is a trust mechanism buried in a 13px paragraph; per-state meaning (held/clearing/frozen/released) isn't explained on /me/tickets.
**Acceptance:** Checkout shows a visual escrow timeline in a highlighted callout; ticket status badges have hover/expandable per-state explanations and an estimated release date when clearing; refund-policy link is prominent.

### WEB-5 — KYC form client-side validation + color-coded status  [P2 · M · web]
**Rationale:** Organizer payout KYC (IBAN/SWIFT/name) has no client validation and text-only status; bad bank details silently block payouts.
**Acceptance:** Inline validation (IBAN 15-34 + country prefix, SWIFT 8/11, name required) with helper text and field errors; KYC status color-coded (approved=green/pending=amber/rejected=red) with submitted date; editable unless approved.

### WEB-6 — Replace prompt()-based report/transfer/delete with accessible modals  [P2 · M · web]
**Rationale:** Report (EventDetailClient), transfer and account-delete (me/tickets, account) use window.prompt() (verified) — not screen-reader friendly and a WCAG gap on core flows.
**Acceptance:** ReportModal, TransferTicketModal, DeleteAccountModal with form fields, validation, and focus management replace the prompts; keyboard and screen-reader operable.

### CROSS-1 — Age-gate hardening: prominent gate + enforce at order creation  [P1 · M · cross]
**Rationale:** Web skips the age check when unauthed so a minor can proceed toward payment and fail late; Android shows the gate as silent text. Compliance/liability risk agreed by web+android+fullstack audits.
**Acceptance:** Age-restricted events show a prominent badge/banner on card and detail; unauthed users are prompted to sign in before checkout; too-young authed users get a clear message + link to set DOB; order creation (not just checkout) rejects underage. Web and Android both updated.

### BACK-1 — Guard or transfer escrow on paid-ticket transfer  [P1 · M · backend]
**Rationale:** transfer_ticket (api/src/tickets.rs) reassigns ticket owner_id but does not move or guard the order/escrow (verified) — original buyer keeps money-at-risk for a ticket they no longer hold; financial-visibility leak.
**Acceptance:** Transfer of a ticket with non-null escrow_status is either blocked with a clear message OR moves the order/escrow ownership atomically; behavior documented and surfaced in transfer UI.

### AND-1 — Android rich empty/error states with Retry + status palette parity  [P2 · M · android]
**Rationale:** All screens show a bare CircularProgressIndicator and text-only empty/error states with no retry; status badges should adopt the shared palette+glyphs from FS-1.
**Acceptance:** Skeleton/shimmer loading, icon+heading+subtext empty states with CTA, Snackbar errors with Retry; StatusBadge uses shared palette and a colorblind-safe glyph/pattern.

### AND-2 — Android accessibility pass: contentDescription + non-color status cues  [P2 · L · android]
**Rationale:** Icons/images lack contentDescription and status relies on color alone; TalkBack users are excluded (legal + ethical).
**Acceptance:** All meaningful Icons/Images have contentDescription; status conveyed by glyph/text in addition to color; a documented TalkBack checklist passes on Афіша, detail, tickets, create-event.

### ADM-9 — Admin login + form polish (branding, focus, required indicators)  [P2 · S · admin]
**Rationale:** Login is a tiny card on empty dark background (looks like a phishing-prone demo); admin forms have muted labels, no required markers, faint focus.
**Acceptance:** Login has Underground Admin branding and a forgot-password link; form labels are full-contrast with required asterisks; inputs have a clear focus state; button hover/active states present.

