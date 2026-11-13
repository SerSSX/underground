
## 2026-11-12 — GitHub repository made public (CI minutes)

Context: the SerSSX account's 2000 free Actions minutes for November ran out (shared with msgview); every
Underground and msgview CI job was refused before start. Options put to the owner: a $10 Actions spending limit,
make the Underground repo public (unlimited free minutes, incl. arm64 runners), or wait until 1 Dec.
Owner's answer (verbatim): «Зробити репо Underground публічним».
Before the switch: all 7 remote branches' full history scanned for committed secret files (.env, keys, keystores,
credentials) and secret patterns (Stripe/AWS/GitHub/Google/SendGrid/private keys/JWTs) — none found.
Consequence: the code, docs and history are readable by anyone; secrets stay out of the repo as before (values
live only on the box / in GitHub secrets).

## 2026-11-13 — OpenStreetMap data under ODbL accepted

Context: TASK-303 stores OSM-derived cities (names, coordinates) learned from Photon and publishes them via
/api/cities; accurate time zones from coordinates need tzf-rs, whose bundled boundary data is ODbL-1.0.
Options put to the owner: accept ODbL (attribution "© OpenStreetMap contributors"; a publicly shared database
derived from OSM stays under ODbL) or do not store OSM data (no learned cities, country-table zones only).
Owner's answer (verbatim): «Так, приймаємо ODbL (Recommended)».
Consequences: tzf-rs (MIT code + ODbL data) may be adopted; OSM attribution shown site-wide; the learned city
list stays open (it is public through /api/cities already).

## 2026-11-13 — TASK-304: three upload tests changed for WebP output (owner approval, §5)

Behaviour change asked by the owner (TASK-304 item 4): every upload is re-encoded to WebP.
- `api/tests/uploads.rs::upload_then_serve` — was: URL ends `.png`, served bytes equal the input; now: URL ends
  `.webp`, served bytes start with the RIFF/WEBP magic; why: uploads are no longer stored as sent.
- `api/tests/uploads.rs::upload_accepts_jpeg_and_webp` — was: fake bytes named `.jpg` accepted; now: real JPEG and
  WebP images, URL ends `.webp`; why: fake bytes are now rejected by content sniffing (a stricter control).
- `api/tests/uploads_errors.rs` — the fake PNG payload replaced by a real PNG; assertions unchanged.
Owner's answer (verbatim): «Так, погоджую (Recommended)».

## 2026-11-13 — TASK-304: outbound mail budget (orchestrator reading, reversible)

The review found that real mail (reset links, login codes) can be sent to any address. Reading taken without
asking (cheap to change, env-configurable): a global daily send budget (`MAIL_DAILY_BUDGET`, default 80, below
Resend's free 100/day) and a per-recipient daily cap; when the budget is spent the API refuses new mails with an
honest "mail is temporarily unavailable" instead of claiming it was sent. Per-address limits stay strict: an
attacker can delay a victim's codes/reset by spamming that address, a known trade-off versus mail-bombing; a
password account can still sign in with its password.
