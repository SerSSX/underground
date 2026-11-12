
## 2026-11-12 — GitHub repository made public (CI minutes)

Context: the SerSSX account's 2000 free Actions minutes for November ran out (shared with msgview); every
Underground and msgview CI job was refused before start. Options put to the owner: a $10 Actions spending limit,
make the Underground repo public (unlimited free minutes, incl. arm64 runners), or wait until 1 Dec.
Owner's answer (verbatim): «Зробити репо Underground публічним».
Before the switch: all 7 remote branches' full history scanned for committed secret files (.env, keys, keystores,
credentials) and secret patterns (Stripe/AWS/GitHub/Google/SendGrid/private keys/JWTs) — none found.
Consequence: the code, docs and history are readable by anyone; secrets stay out of the repo as before (values
live only on the box / in GitHub secrets).
