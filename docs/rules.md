# docs/rules.md — Underground project policy

**General process lives in `~/.claude/rules/rules.md` (always loaded).** It wins over this file, except where
this file is stricter: then both bind. The previous project doctrine (v0.5.0, 42 KB) is archived at
`~/Documents/rules-archive-20261102-pre-v3.13/repos/underground/rules.md`; its stricter project rules are kept
below and in `CLAUDE.md`.

## Project policy (stricter than the general file — binds in addition)

- **Backlog first:** every TASK or PROJECT gets `docs/backlog/TASK-###-*.md` before analysis or code; anything not
  estimable as one small unit is decomposed into small leaves before code.
- **Speck per task:** `docs/specks/SDD-###-*.md` with `AC-1..n`, `SEC-1..n` and a test matrix, checked before code.
- **Spec is truth:** behaviour changes go to `docs/spec.md` first, then tests, then code.
- **Code limits:** no `any`/unsafe casts, no placeholders/TODO/dead code; ≤120 chars per line, ≤30 lines per
  function, ≤500 lines per type; no resource leaks.
- **Tests:** container-first; 100 % unit + integration coverage; every suite has positive, negative and [SEC]
  cases. Never lower the CI-enforced thresholds.
- **Money:** all money logic goes through the single seam `api/src/escrow.rs`.
- **Owner gates (kept from v0.5.0):** a MEDIUM task is flagged and confirmed with the owner before it
  starts; an estimate is not changed after work starts without approval; code that cannot meet the limits
  above needs approval; commit/push only on the owner's request.
- **Tests (kept from v0.5.0):** an ephemeral DB per run; curl tests cover 100 % of public routes (positive,
  negative, [SEC]); security tests are titled `[SEC][UC-###]`; no flaky tests; no network access outside
  the test scope.
- **Needs the owner's approval:** a breaking API change, env/secret changes, CI changes not asked for.
  Forbidden: hardcoded ports/CORS/redirects, skipped tests, direct DB mutation outside migrations.
