# TASK-301 — Step 0: CI and the DEV stand on the shared box

Status: IN PROGRESS (2026-11-12)
Lane: TASK (rules.md §0), part of TASK-300 plan step 0 · brief mode: pro (no gaps: plan approved)
Parent: [TASK-300](TASK-300-mvp-free-tickets.md) · plan: `docs/analysis/2026-11-11-mvp-gap-and-plan.md` §3 step 0

## Request (owner, 2026-11-12, verbatim)

> Так, дивись, у нас є сервер, на якому можна бекенд розвернути. Туди його пуш, так сказати. Dev версіях і буде на
> dev URL стандартній нашій, да? І як би от погнали, як то кажуть, в розробку.

Reading recorded: the plan is approved («погнали в розробку»); step 0 deploys the stack to DEV on the shared box
65.108.87.247 at the standard dev URL pattern `<product>-dev.65-108-87-247.sslip.io`
(as `msgview-dev.65-108-87-247.sslip.io`). Prod stays on the owner's command (rules §9).

## Done when

- CI runs on every push and PR: API (fmt, tests against Postgres, cargo audit), web and admin (typecheck, build,
  npm audit high), Android (debug build + unit tests). Green on this branch.
- DEV stand up: `https://underground-dev.65-108-87-247.sslip.io` (web + API same origin) and
  `https://underground-admin-dev.65-108-87-247.sslip.io`; seeded demo data; noindex; memory limits so the
  LIVE neighbours (msgview and others) are not starved.
- One deploy command from the Mac, which refuses a commit whose CI is not green, deploys a clean export of that
  commit, smoke-tests dev, and appends a release record.
- msgview (LIVE, shares the box and Caddy) smoke green after the Caddy change.
