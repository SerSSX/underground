
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
