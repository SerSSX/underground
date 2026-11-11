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

## Review 1 (distinct reviewer, opus/xhigh, of 30f03c0) — triage

| # | Sev | Finding | Decision |
|---|---|---|---|
| 1 | BLOCKER | CI red: fixed fixture dates passed (`events_create`, then `events_search` once `--no-fail-fast` showed it) | fixed: relative dates, assertions unchanged (was: `2026-09-0x`, now: `now()+30..34 days`, same order; why: the afisha hides past events). `--no-fail-fast` in CI |
| 2 | MAJOR | smoke greps `/_next/static/<sha>/`, absent in App Router HTML | fixed: smoke looks for the sha in the page and also fetches one JS chunk (13) |
| 3 | MAJOR | rollback across a migration fails (`VersionMissing`) | fixed: `set_ignore_missing(true)`; restore outcome recorded and printed (12) |
| 4 | MAJOR | admin@ squattable via open registration before seed | fixed: seed logs in all three accounts with box passwords before any UPDATE/hash copy; separate admin password (18) |
| 5 | MAJOR | disk/CPU of LIVE neighbours unprotected | fixed: 4 GB loop fs for data, log rotation, `cpus: 1.0`, tarballs deleted after load, deploy refuses < 3 GB free. Rate limits on register/uploads → step 1 (Б6) |
| 6 | MAJOR | Caddy change not gated by msgview's real smoke, no undo on msgview failure | fixed: origin check from the box before/after, msgview prod-smoke dispatched and watched; red undoes the change (only when this run made it) |
| 7 | MINOR | pipefail + `grep -q` false reds | fixed: smoke in Python, bootstrap greps a variable |
| 8 | MINOR | pruning could drop running/previous images or incoming dirs | fixed: keep current, previous, last 3 green by release record |
| 9 | MINOR | run selection too loose | fixed: `--branch`, `--status success`, the four required jobs must be success |
| 10 | MINOR | default target refused on docs-only tip; rollback dies with artifact | fixed: newest green ancestor; images already on the box are reused |
| 11 | MINOR | empty pre-seed backup possible | fixed: pipefail + non-empty check |
| 12 | MINOR | restore outcome not reported | fixed |
| 13 | MINOR | smoke does not prove UI loads; catalogue time bomb | fixed: chunk fetch; `?past=true` |
| 14 | MINOR | CI: cancelled-run android gap, double PR runs, cancel on main, no gate for deploy/ | fixed: diff vs last green run; push only main + DEV branch; no cancel on main; shellcheck + `compose config` |
| 15 | MINOR | build-time prerender fetch unbounded | fixed: 10 s timeout |
| 16 | TRIVIAL | `version_reports_build` cannot prove the sha is wired | accepted: the deploy smoke proves it against the real sha |
| 17 | TRIVIAL | JWT on a command line; login rate limit lets anyone lock maker@ (red smoke) | JWT fixed (Python). Lockout accepted on DEV: a red smoke restores the previous release, no harm |
| 18 | TRIVIAL | README overstated maker's events; one password incl. admin | fixed |
