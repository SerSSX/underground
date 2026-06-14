# docs/rules.md|~/rules.md — Tight Rules (Spec + Speck Driven Development)

**Version: 0.5.0**

These rules are **STRICT / NON‑NEGOTIABLE** for all AI agents and contributors.

Conflict resolution order:
1. System message → 2. User instruction → 3. `docs/rules.md` → 4. `docs/spec.md` → 5. `docs/specks/*.md` → 6. `docs/backlog/*.md` → 7. `README.md`

If truly ambiguous — ask before acting.

---

## 0) Scope & Applicability

Repo may contain: backend only (NestJS / ASP.NET Core / other), frontend only, or fullstack monorepo.
Apply **only what exists**; preserve **fail-fast test order**.

---

## 1) Top Priorities (Decision Order)

When tradeoffs exist, agents MUST prioritize in this exact order:

1. Business request implemented (matches `docs/spec.md` + task Speck).
2. Quality gate passed (typecheck / lint / format if enforced, tests, coverage, audits).
3. Unit tests green — **100%** lines + branches + functions (API and UI independently).
4. Integration tests green — **100%** lines + branches + functions + classes if measurable.
5. Curl tests cover all positive / negative / security API responses (if API exists).
6. UI tests cover all UCs + edge cases + negative paths + security vectors (if UI exists).
7. Security audit passes — no new high/critical findings, no obvious insecure patterns.
8. Dependencies: latest stable versions used; exceptions justified in Speck.
9. No avoidable speed regressions; obvious hotspots addressed.
10. No avoidable size/bundle bloat.
11. Final gate on **production images**: full stack + all curl + all UI tests pass.
12. CI steps simulated and verified.

---

## 2) Documentation Doctrine (`./docs`)

### 2.1 Required files

- `docs/spec.md` — main spec; business truth (replaces `functional.md` / `task.md`)
- `docs/development-manual.md`
- `docs/installation-manual.md`
- `docs/configuration-manual.md`
- `docs/backlog/` — task backlog (§12)

Deprecated docs (`functional.md`, `task.md`, etc.): migrate into `docs/spec.md`, eliminate contradictions.

### 2.2 `docs/spec.md` is the business truth

Agents MUST NOT implement behavior contradicting `docs/spec.md`.

`docs/spec.md` MUST describe:
1. What the system does
2. What business value it delivers
3. High-level architecture / flow
4. Use cases (grouped logically) with IDs `UC-###`
5. Business rules (`BR-###`) and workflows (`WF-###`)

### 2.3 Use case requirements (mandatory)

Each use case MUST have ID `UC-###` and include **all** of the following sections:

**Summary** — one-sentence description.

**Description** — narrative explaining actors, trigger, and outcome.

**Functional Requirements** — what the system must do from the business perspective:
- Acceptance conditions (user-observable outcomes)
- Data inputs and outputs
- Validation rules, constraints, authorization rules
- Failure modes (validation / auth / empty state / conflict — user-visible messages)

**Technical Requirements — Backend** (omit if no backend is involved):
- API endpoints: method, path, request/response shape, status codes
- Data model changes (entities, fields, relations, migrations)
- Business logic and service-layer responsibilities
- Validation and error handling at API level
- Security constraints (auth, RBAC, rate limiting)

**Technical Requirements — Frontend** (omit if no frontend is involved):
- UI screens / components involved
- User interactions and navigation flows
- State management and data-binding requirements
- API calls made (reference backend endpoints above)
- Responsive / breakpoint behavior
- Accessibility requirements

**Flow (Mermaid)** — mandatory diagram of the full UC flow.

**Schematic layouts** — per screen under `docs/ui-schematics/UC-###/<screen>.{txt|svg|mermaid}` (if UC has UI; see §4.1.2).

**Baseline screenshots** — per Playwright project under `docs/ui-baselines/UC-###/<project>/<step>.png` (see §4.1).

**Related BR/WF** — list of `BR-###` / `WF-###` IDs that apply.

### 2.4 Business rules / workflows (mandatory)

- `BR-###` — invariants / constraints
- `WF-###` — multi-step processes

Use cases MUST reference applicable BR/WF IDs.

---

## 3) Mandatory Speck Driven Development (SDD)

### 3.1 Specks location

Every task MUST have: `docs/specks/SDD-###-short-title.md`

**Prerequisite:** task MUST pass Backlog Lifecycle (§12) and be estimated `LOW` before Speck creation.

### 3.2 Speck audit (mandatory before any code change)

1. Read `docs/spec.md` — identify impacted `UC-###`, `BR/WF`, API contracts, baselines.
2. Determine if the request can break existing UC flows, BR/WF invariants, API contracts, UI baselines.
3. Create/update Speck: scope + non-goals, `AC-1..n`, `SEC-1..n` (security ACs mandatory), failure modes, Test Matrix mapping each AC/SEC → test stages, impacted IDs.
4. If behavior changes: update `docs/spec.md` first (and baselines if UI-visible), then tests, then code.

**No silent drift:** if implementation reveals missing/incorrect requirements, update Speck (and `docs/spec.md`) BEFORE changing code.

---

## 4) Spec Reconstruction + Validity Checks

### 4.1 Missing `docs/spec.md` — reconstruct (mandatory)

If `docs/spec.md` is missing, reconstruct it before implementing any behavior change.

Reconstruction MUST:
- Create `docs/spec.md` following §2 (including all UC sections per §2.3)
- Derive behavior from actual code (and OpenAPI if present)
- Derive UCs from routes / pages / workflows / background jobs / persistence constraints
- Include Mermaid diagrams, schematics, and baseline screenshot references for every UC
- Generate baselines via Playwright if UI is runnable in Docker; report what is missing if not

Reconstruction MUST NOT invent business intent not supported by code.

### 4.1.1 Reconstruction is a task — decompose it

Reconstruction follows Backlog Lifecycle (§12) and SDD (§3). For non-trivial codebases, decompose into sub-tasks:

- One sub-task per major subsystem / module / bounded context
- One sub-task per UI area (auth, dashboard, settings, admin, etc.)
- One sub-task for API contract inventory (routes, methods, status codes, response shapes)
- One sub-task for BR/WF extraction
- One sub-task for UI design token extraction (§4.1.3)

Leaf sub-tasks MUST be estimated `LOW` or `VERY LOW` before work begins.

### 4.1.2 Screen reconstruction — schematics required

Each UC screen MUST have a schematic under `docs/ui-schematics/UC-###/<screen-name>.{txt|svg|mermaid}`.

Schematics MUST show: regions/zones, major UI elements (inputs, buttons, tables, nav, toolbars), grouping, tab order, interactive element annotations (e.g., `[Submit — POST /api/orders]`), and responsive breakpoint behavior.

Multiple screens in a UC flow → numbered files: `01-list.txt`, `02-detail.txt`, `03-confirm-dialog.txt`.

`docs/spec.md` MUST reference schematics by path.

### 4.1.3 UI reconstruction — design tokens extraction

When reconstruction involves a UI codebase, extract `docs/ui-design-tokens.md` including:
- **Colors:** primary, secondary, accent, error/warn/success/info, neutral, background — hex/RGB + CSS variable names
- **Typography:** font families, size scale, weights, line heights, letter spacing
- **Spacing/sizing:** base unit, padding/margin scale, breakpoints (px), max-widths, grid details
- **UI element blueprints:** recurring components (buttons, inputs, cards, modals, nav, tables, chips) — variants, states, dimensions
- **Iconography:** library, sizes, usage conventions
- **Border/shadow:** radii, widths, elevation values
- **Motion:** durations, easing, transition patterns

Extraction MUST come from actual code — not invented. Document inconsistencies explicitly.

### 4.2 Spec audit against codebase (on user request)

1. Extract UC / BR/WF / API contract / baseline inventory from `docs/spec.md`.
2. Inspect codebase (and OpenAPI if present).
3. Generate timestamped audit report: `docs/spec-audit-<YYYY-MM-DDTHH-MM-SS>.md`
   - MUST NOT overwrite `docs/spec.md`, existing audit reports, or baselines
   - Report must diff spec vs code: missing UCs, contract mismatches, flow mismatches, BR/WF mismatches, baseline gaps, auth/security mismatches
   - Each finding: what spec says / what code does / divergence category
4. Inform user; user decides resolution path.

Agent MUST NOT auto-change `docs/spec.md` or code after the audit.

---

## 5) Code Quality Rules

### 5.1 Type safety (hard)

**TypeScript:** NEVER use `any`; NEVER use casts to bypass correctness; prefer interfaces, generics, discriminated unions, exhaustive `never` checks.

**C#/.NET:** Avoid `dynamic`; no unsafe casts (use pattern matching / `TryParse`); respect nullable reference types.

Violation = failed task.

### 5.2 Maintainability limits (hard)

- Max line length: **120 chars**
- Max method/function: **30 non-empty, non-comment lines**
- Max class/type: **500 non-empty, non-comment lines**

If touched code already violates limits: do not refactor unrelated code; prefer minimal extraction with identical behavior; request explicit approval if change cannot be done within limits.

### 5.3 Implementation quality

- Complete working code — no placeholders, no TODOs, no dead code, no commented-out logic
- Minimal, scope-limited changes; follow existing conventions
- No reduction in security, validation, or observability
- **No resource leaks:** close/dispose all connections, streams, file handles, timers, and event listeners (`try/finally`, `using`, `defer`, or framework patterns)

---

## 6) Testing Doctrine (Container-first, Fail-fast)

### 6.1 Test change policy (hard)

Adding tests is always allowed. Modifying/removing existing tests is FORBIDDEN without explicit user approval.

### 6.2 Container-first (hard)

ALL tests run in Docker containers. No host dependencies. Required infra (DB/cache/queue/storage) must be dockerized.

### 6.3 Ephemeral DB (hard)

For any DB-backed test suite: DB starts empty every run; no persisted volumes; readiness gates required; bounded retries; migrations/seeds after readiness; tests order-independent.

### 6.4 Positive + negative + security testing (hard)

Every suite (unit, integration, curl, UI) MUST cover:
- **Positive paths:** expected inputs → expected outputs
- **Negative paths:** invalid/missing/malformed inputs rejected correctly
- **Security/adversarial** (mandatory):
  - Injection (SQL, NoSQL, XSS, command, LDAP)
  - Auth bypass (missing/expired/forged tokens, tampered JWTs, replay)
  - Authorization escalation (IDOR, privilege escalation)
  - Input abuse (oversized payloads, nested JSON, boundary values, null bytes, unicode)
  - Rate limiting / brute-force (if applicable)
  - Header manipulation (host injection, missing security headers, CORS abuse)
  - Path traversal
  - Mass assignment / parameter pollution
  - **Memory leaks** (unclosed connections, unbounded caches, listener accumulation)

Security tests MUST be explicitly labeled: `[SEC][UC-###]` in test titles.

### 6.5 Fail-fast stages (mandatory)

1. Unit (positive + negative + security)
2. Integration (positive + negative + security)
3. Curl — Dev/Runtime images (positive + negative + security)
4. Base UI — Playwright, Dev/Runtime images
5. UI — Playwright headless Chromium, Dev/Runtime images (positive + negative + security)
6. Curl — Production API image (100% API coverage)
7. Production — optimized fullstack + all curl + all UI tests

CI MUST stop at first failing stage. Security tests run **within** each stage, not isolated to a late stage.

---

## 7) Coverage + Curl + UI Requirements

### 7.1 Coverage (hard)

- Unit: **100%** lines + branches + functions (API and UI independently)
- Integration: **100%** lines + branches + functions + classes if measurable

No meaningless assertions. 100% coverage with only happy-path assertions = non-compliant.

### 7.2 Curl tests (if API exists)

MUST cover 100% of public routes (source: OpenAPI → `docs/spec.md` → route inventory). Per route: assert status code + response body shape/content + primary business outcome. Cover all positive, negative, and security scenarios (§6.4). Run at stages 3, 6, and 7.

### 7.3 UI tests — mandatory device matrix

**Device matrix:**
- Desktop
- Tablet portrait + landscape
- Mobile portrait + landscape: last 3 iPhones, last 3 Pixels, last 3 Samsung flagships (ALL models in those generations)

Matrix defined in `docs/configuration-manual.md`. Playwright MUST implement as **projects**. Every test runs across ALL projects unless explicitly scoped and approved.

### 7.4 UI test mapping + baselines (hard)

- Every UI test MUST map to a `UC-###` (included in test title: `[UC-###]`)
- Every `UC-###` MUST be covered by at least one Playwright test with recorded video
- Tests MUST: do real interactions (click/type/submit/scroll), prove elements interactable, produce step screenshots + video, compare against baselines per UC step per project, include negative and security tests

**Human-like interaction speed (hard):** 2–3 chars/sec typing (300–500ms keystroke delay via `type()` + `delay`, NOT `fill()`); ≥1s after navigation; 300–500ms before/after clicks; 200–400ms between fields; 1–2s after submit; 500ms–1s after scroll. Implement via shared utilities (`humanType()`, `humanClick()`, `waitForHuman()`) — no ad-hoc sleeps.

**Cursor visibility (hard):** videos MUST show cursor position and click events. Use Playwright cursor visualization or inject a cursor overlay (CSS/JS) with `mousemove`/`click` tracking.

**Video resolution (hard):** minimum **1920×1080 (Full HD)** preferred; 1280×720 acceptable only if Full HD causes instability (justify in Speck). Configure `video.size` explicitly. Mobile/tablet: match device viewport (never below 720p height).

### 7.5 Artifacts (hard)

Every Playwright run MUST produce: HTML report, step screenshots, video per test (Full HD, cursor visible). On failure: screenshots + video (mandatory) + traces (strongly recommended). Paths: `playwright-report/`, `test-results/`.

### 7.6 Customer Demo test (hard)

One final UI test covering ALL use cases: runs last, all UC IDs in title `[DEMO][UC-###]...`, full device matrix, human-like speed, Full HD with visible cursor.

---

## 8) Dev/Build Infrastructure Rules

### 8.1 One-command dev mode

Root `docker-compose.yml`: `docker compose up --build` brings up ALL services, seeded and ready. Seeding idempotent.

**Port management (hard):** expose only required ports; all ports configurable via `.env`; auto-resolve conflicts (detect error → pick available port → update `.env` → retry; do not ask user); `docker-compose.yml` references port vars from `.env`.

### 8.2 Dev data + temp dirs (hard)

- `./.dev-data/` — dev volumes (delete to reset system)
- `./.dev-temp/` — build-time temp (safe to delete)

Tests MUST NOT rely on `./.dev-data/`.

### 8.3 Container image policy (hard)

Multi-stage builds required. **Production stage priority:**
1. **Distroless** (preferred) — `gcr.io/distroless/nodejs-debian12`, etc.
2. **Alpine bare** — if distroless is not viable
3. **`xx-alpine` variant** — last resort (`node:22-alpine`, `nginx:alpine`, etc.)

Lower-priority choice MUST be justified in Speck. Always use **latest LTS** pinned to specific minor/patch (e.g., `node:22.14-alpine`, not `latest`). Production containers: non-root user + `HEALTHCHECK` + `.dockerignore`.

### 8.4 UI framework (when building from scratch)

Preferred: **Latest Angular LTS** + **Angular Material** + **Zoneless** (signals) + **Standalone components**.

**Runtime config (hard — all frameworks):** UI loads all env-dependent settings from `settings.json` served from web root. Fetch at startup before any service uses those values. **Rebuilding for different environments is FORBIDDEN** — switch by replacing `settings.json` only.

**Server-generated `settings.json` (preferred):** NestJS / ASP.NET Core serves `settings.json` dynamically from its own config — unauthenticated, cacheable. Static fallback MUST still exist for standalone UI dev.

### 8.5 Reverse proxy support (hard)

MUST work behind nginx/Traefik/similar: respect `X-Forwarded-For/Proto/Host/Prefix`; no hardcoded host/port; UI base href configurable; WebSocket proxying if applicable; CORS env-driven; auth callbacks use externally visible URL; health checks accessible without auth.

Proxy in repo: prefer Traefik or nginx, config env-driven. No proxy: document expected setup in `docs/installation-manual.md`.

### 8.6 CI simulation (mandatory)

Agents MUST simulate CI locally (container-first) before declaring done. Repo SHOULD provide `make ci` or `./scripts/ci.sh` running typecheck/lint → unit → integration → curl → UI → production → security/dependency checks.

---

## 9) Security, Dependencies, Performance, Size

### 9.1 Security audit (every task)

Run relevant checks. No new high/critical vulnerabilities. No obvious insecure patterns. Report pre-existing issues explicitly — do not hide.

### 9.2 Dependencies (every task)

Base images: latest LTS (§8.3). New dependencies: latest stable. Existing: update if safe and in scope. If not updating: record what is outdated, why not updated, and upgrade path in Speck.

### 9.3 Speed and size

No avoidable regressions or bloat. Production images follow §8.3. Define and enforce budgets in `docs/configuration-manual.md`.

---

## 10) Forbidden Actions (Hard)

- Breaking API changes without explicit approval
- Changing env vars/secrets without explicit approval
- Hardcoding ports in `docker-compose.yml`
- Building environment-specific UI artifacts (MUST use runtime `settings.json`)
- Hardcoding CORS origins, base URLs, or redirect URIs
- Modifying CI/CD unless explicitly requested
- Skipping tests, lowering coverage thresholds, disabling reporting
- Ignoring failing tests or introducing flaky tests
- Direct DB mutation outside migrations/seeds
- Network access outside test scope
- Implementing without a `docs/backlog/TASK-###-*.md` file
- Implementing a `HIGH` or `IMPOSSIBLE TO ESTIMATE` task without decomposition

---

## 11) "Done" Definition

Repo is done only if all of the following exist and pass:

**Docs:** `docs/spec.md`, `docs/development-manual.md`, `docs/installation-manual.md`, `docs/configuration-manual.md`, `docs/specks/`, `docs/backlog/` (all tasks `DONE`/`REJECTED`/`DUPLICATE`), `docs/ui-baselines/`, `docs/ui-schematics/` (if UI), `docs/ui-design-tokens.md` (if UI).

**Infra:** root `docker-compose.yml`; `.env` with safe defaults (no real secrets), all ports configurable; `.dockerignore`; `settings.json` (if UI, server-generated preferred); reverse proxy support (§8.5); `.dev-data/` + `.dev-temp/` conventions honored; production images per §8.3.

**Tests:** all stages pass fail-fast; 100% unit + integration coverage; 100% curl route coverage + positive/negative/security; UI covers all UCs + edge cases + negatives + security + device matrix + demo test; every UC has Playwright video (Full HD, cursor visible, human-like speed); security tests labeled `[SEC]` at every applicable stage.

---

## 12) Task Backlog & Estimation Lifecycle

### 12.1 Naming

`docs/backlog/TASK-###-short-title.md` — sequential, zero-padded to 3 digits. Create file BEFORE any analysis or code change.

### 12.2 Intake

Record user request verbatim in `## Raw Request`. Set status: `INTAKE`.

### 12.3 Refinement (mandatory — no skipping)

Cross-check against codebase and docs:
1. Read `docs/spec.md` — impacted UC/BR/WF, API contracts, UI baselines
2. Read `docs/backlog/` — detect duplicates, dependencies, conflicts
3. Check feasibility vs current architecture
4. Write `## Refined Description`: scope, non-goals, impacted IDs, dependencies, risks
5. Status → `REFINED`

If ambiguous/contradictory/incomplete: ask user → status `REFINED:BLOCKED` until resolved.

### 12.4 Estimation

| Level | Definition |
|---|---|
| **VERY LOW** | Trivial; single file; no new tests beyond existing patterns; no spec changes. <30 min. |
| **LOW** | Small, well-scoped; ≤3 files; clear ACs; predictable test surface. ≤2h. |
| **MEDIUM** | Multi-file; new UC or BR possible; new tests + Speck; moderate risk. ≤8h. |
| **HIGH** | Large scope; multiple UCs/BRs; new infra or significant refactoring; hard to verify atomically. |
| **IMPOSSIBLE TO ESTIMATE** | Scope unclear even after refinement; requires spike, prototype, or user decision. |

Record level + 1–3 sentence justification. Status → `ESTIMATED`.

### 12.5 Implementation gate (hard)

- `VERY LOW` / `LOW`: proceed to Speck + implementation.
- `MEDIUM`: proceed but flag to user and confirm before starting.
- `HIGH` / `IMPOSSIBLE TO ESTIMATE`: MUST decompose (§12.6). Implementation FORBIDDEN until all leaf sub-tasks are `LOW`.

### 12.6 Task decomposition (mandatory for HIGH / IMPOSSIBLE TO ESTIMATE)

Recursively decompose along natural boundaries:
- Distinct UCs (one sub-task per UC — see also §12.6.1 for refactoring)
- Distinct layers (API vs UI vs infra)
- Distinct BRs/WFs
- Sequential phases (schema → API → UI integration)

Sub-task naming: `TASK-###-A-short-title.md`; nested: `TASK-###-A1-short-title.md`.

Each sub-task: full lifecycle (intake → refinement → estimation). Decomposition stops when ALL leaf sub-tasks are `LOW` or `VERY LOW`. Parent → `DECOMPOSED`; only leaf sub-tasks are implemented directly. Document execution order and dependencies between sub-tasks.

#### 12.6.1 Refactoring / reimplementation decomposition (mandatory)

When a task involves refactoring or reimplementing in another technology/stack (see also §13), the decomposition MUST cascade to the level of individual use cases. For each UC in scope, create **separate** sub-tasks for each applicable layer:

- `TASK-###-UC###-BE` — backend reimplementation for `UC-###` (if backend is involved)
- `TASK-###-UC###-FE` — frontend reimplementation for `UC-###` (if frontend is involved)
- `TASK-###-UC###-INT` — BE/FE integration for `UC-###` (if both layers exist)

Multiple sub-tasks per UC are normal and expected when a UC spans complex logic across layers.

All UC-level sub-tasks MUST follow the full lifecycle and each MUST be estimated independently. No migration sub-task may begin until the UC-level decomposition is complete and all leaves are `LOW`.

### 12.7 Implementation execution

1. Create Speck (`docs/specks/SDD-###-*.md`) per §3
2. Perform Speck audit per §3.2
3. Implement per §5–§11
4. Update backlog status: `IN PROGRESS` → `DONE`
5. When all leaf sub-tasks of a parent are `DONE`: verify parent ACs end-to-end → parent → `DONE`

### 12.8 Task statuses

| Status | Meaning |
|---|---|
| `INTAKE` | Raw request recorded, not refined |
| `REFINED` | Cross-checked against spec, scope clarified |
| `REFINED:BLOCKED` | Ambiguities found; awaiting user clarification |
| `ESTIMATED` | Size estimated, awaiting gate decision |
| `DECOMPOSED` | Too large; broken into sub-tasks |
| `READY` | Passed gate; ready for Speck + implementation |
| `IN PROGRESS` | Speck created; implementation underway |
| `DONE` | Implemented, all tests pass, all rules satisfied |
| `REJECTED` | Not proceeding (reason documented) |
| `DUPLICATE` | Duplicate of another task (reference documented) |

### 12.9 Forbidden backlog actions (hard)

- Starting implementation without refinement and estimation
- Implementing `HIGH` / `IMPOSSIBLE TO ESTIMATE` without decomposition
- Skipping refinement
- Deleting backlog files (mark `REJECTED` or `DUPLICATE`)
- Changing estimation after implementation starts without user approval

---

## 13) Code Migration / Rewrite Rules

### 13.1 Scope

Applies whenever the user asks to migrate, rewrite, or port code to a new technology, framework, language, or architecture.

**Migration = complete drop-in replacement.** The result MUST be indistinguishable from the original to every consumer:

- **API parity (1:1):** every route, HTTP method, request shape, response shape, status code, error message, and header contract MUST be reproduced exactly. No consumer (curl scripts, UI, external integrator) may need any change after the swap.
- **UI parity (1:1):** every screen, navigation flow, interaction, element, label, validation message, error state, and visual behavior MUST be reproduced exactly. No user should notice any difference after the swap. For **TUI** (terminal UI) tools: color palette, layout, widget positions, border styles, and **render order** (the sequence in which regions and widgets are drawn) are all part of the parity contract — pixel-level color screenshots of source and output MUST match.
- **Integration parity:** the BE/FE contract (API calls, payload shapes, auth flows, error handling) MUST be preserved end-to-end.

Any divergence — even a renamed field, reworded error, or reordered response key — is a **migration defect**. Improvements belong in Phase 3 (§13.5) or as separate post-migration tasks.

A rewrite is a full migration where the source is replaced rather than run in parallel.

### 13.2 Documentation prerequisite (mandatory — the contract that defines parity)

**Documentation is not a formality for refactoring — it is the contract.** Parity can only be verified against a precise written specification. Without complete documentation, "done" has no definition and migration has no finish line.

**Migration MUST NOT start until `docs/spec.md` is complete, accurate, and accepted by the user.** Every UC in scope must have all §2.3 sections filled:
- Functional requirements (acceptance conditions, data, validation, failure modes)
- Technical requirements — Backend (endpoints with full request/response shapes and status codes, data model, service logic, security)
- Technical requirements — Frontend (screens, interactions, navigation, API calls, responsive behavior)

The spec is the **sole reference for what constitutes parity**. If a behavior is undocumented, the agent MUST document it (derived from source code) before migrating it.

**Consequence:** an incomplete or vague spec makes migration impossible to declare done. Agents MUST refuse to start migration on any UC whose spec section is not complete and accepted.

If `docs/spec.md` is missing or incomplete: reconstruct per §4.1 as a separate task (§12). Only after user acceptance may migration planning proceed.

### 13.3 Test suite gates for migration (mandatory)

Migration uses two separate test gates that apply at different points in time.

#### 13.3.1 Pre-migration gate — curl tests (mandatory before any migration begins)

**A complete curl test suite MUST exist and pass against the source before any migration work begins.** Curl tests are the API parity harness — defining correctness for every endpoint across every hop.

- **Curl MUST achieve 100% route coverage** — every endpoint, every status code, every positive, negative, and security scenario (§6.4 + §7.2).
- These tests run against the source first, then MUST continue to pass against the output at each hop.
- If curl tests are missing: create a dedicated backlog task (§12) to write them; that task MUST reach `DONE` before any migration sub-task is created and MUST be referenced in the Migration Speck (§13.11).

**Agents MUST refuse to start any migration hop without a passing, complete curl suite against the hop's source.**

#### 13.3.2 Post-migration gate — Playwright UI parity (mandatory after full hop is complete)

**Playwright is the UI parity tool. It runs after the full hop (all UCs migrated, all target unit/integration tests passing) — not before migration begins.** Requiring full Playwright parity upfront would block progress for too long before any UC is delivered.

- During migration: Playwright tests MAY be written incrementally per UC as each UC sub-task completes (encouraged, not required mid-hop).
- **At hop completion:** the full Playwright suite MUST run against both source and output and produce identical outcomes — screen states, flows, interactions, visual baselines, failure modes, across all devices (§7.3).
- **100% UC coverage** required at hop completion — every UC, every flow, every edge case and failure mode (§7.4).
- Tests written per-UC during migration count toward the hop-completion suite.
- Any UI divergence = migration defect; resolve before declaring the hop done.

#### 13.3.3 Source codebase coverage exemption

Unit and integration tests are **not required at 100%** on the source prior to migration. Curl + Playwright/TUI screenshots are the sufficient parity harness. However:
- Existing unit/integration tests MUST stay green and MUST NOT be deleted or weakened.
- The **target** codebase MUST reach full 100% unit + integration coverage (§7.1) — the exemption is source-only.

#### 13.3.4 TUI tools — color screenshot parity (mandatory when a TUI is involved)

When the migrated codebase includes a **TUI (terminal UI)** — any tool that renders to a terminal using colors, box-drawing characters, widgets, panels, or structured layout — these rules replace/extend §13.3.2 for the UI layer:

- **Color screenshots are the parity artifact.** Capture full-terminal color screenshots (ANSI/true-color rendered via a headless terminal emulator or the TUI framework's own snapshot API) of the source at each UC step; capture identical screenshots from the output. These are the baselines stored under `docs/tui-baselines/UC-###/<step>.png`.
- **Render order is part of the parity contract.** TUI frameworks draw regions in a defined sequence; a widget drawn later may overdraw an earlier one. The order in which panels, borders, content areas, status bars, and overlays are rendered MUST be identical between source and output. Any reordering is a parity defect — it diverges under edge conditions (resize, partial render, slow terminal) even if the steady-state screen looks similar.
- **Comparison is character-cell-level:** every cell (glyph + foreground color + background color + attributes: bold/dim/italic/blink) MUST match for each UC step.
- Screenshots MUST be captured at identical terminal dimensions (columns × rows) for source and output — document the dimensions in the Migration Speck.
- **Timing:** same as §13.3.2 — captured and compared at hop completion; incremental per-UC captures during migration are encouraged.
- The Migration Speck MUST record the screenshot tool and capture command for reproducibility.

### 13.4 Feasibility assessment (mandatory first planning step)

Assess whether **direct migration** (source → target in one step) is feasible. Direct migration is prohibited when:
- Source and target languages/paradigms are too semantically distant (e.g., WordPress PHP → Rust)
- Codebase size/complexity makes single-step rewrite impossible to verify atomically
- Behavioral surface area is too large to validate parity in one step

If not feasible: design a **stepping-stone path** (§13.7) before any work begins.

Any migration step is high risk/complexity. Every step MUST be decomposed (§12) until each sub-task is `LOW`.

### 13.5 Three-phase discipline (mandatory per hop)

#### Phase 1 — Direct rewrite (drop-in replacement parity)

Goal: fully working product in the target technology that is a **complete drop-in replacement** — 1:1 API parity, 1:1 UI parity, 1:1 integration parity (§13.1).
- Migrate class by class (§13.9)
- MUST NOT optimize, refactor, add abstractions, alter API contracts, rename fields, reword errors, reorder response keys, change navigation flows, add features, or fix bugs (unless a bug blocks parity — document in Speck)
- Every divergence from the source, however small, is a defect
- Complete only when the full test suite passes on **both** old and new with identical outcomes for every positive, negative, and security case

#### Phase 2 — Dependency replacement

Goal: replace self-written code with battle-tested libraries/framework idioms.
- Replace one module at a time; parity tests must still pass after each replacement
- MUST NOT optimize for performance
- Complete when no further replaceable code remains (or Speck scope exhausted)

#### Phase 3 — Optimization

Goal: improve performance, size, and architecture on a stable, fully covered codebase.
- Only after Phase 2 verified complete
- Each optimization covered by a regression test
- Architecture changes (monolith → services, sync → async) belong here — not in Phase 1 or 2

### 13.6 Missing dependency — cascade rewrite (mandatory)

If a dependency used by the source codebase has **no equivalent in the target technology**, it MUST be rewritten in the target technology as part of the migration. Substituting a different library, silently dropping behavior, or leaving the dependency unported are all forbidden.

Rules:
- Before starting a UC sub-task, the agent MUST inventory all dependencies of that UC's code and classify each as: **available** (direct equivalent exists in target), **replaceable** (a battle-tested alternative covers the same contract), or **missing** (no equivalent; must be rewritten).
- Each **missing** dependency MUST be treated as a separate migration task and decomposed per §12 and §13.8, following the same three-phase discipline (§13.5).
- The cascade applies recursively: if the missing dependency itself has sub-dependencies missing in the target, those MUST also be rewritten.
- Missing dependency tasks MUST be completed (Phase 1 parity verified) before the UC sub-task that depends on them can begin.
- The dependency cascade inventory MUST be documented in the Migration Speck (§13.11) — each dependency with its classification and, for missing ones, its own sub-task ID.
- Dependency rewrites MUST achieve 100% behavioral parity with the original — they are Phase 1 migrations, not reimaginings. Phase 2/3 may follow the same hop schedule as the parent migration.

### 13.7 Stepping-stone migration path (mandatory when direct is infeasible)

Design a path before any work begins. Choose intermediate technologies to minimize semantic distance per hop. Each hop MUST be independently deliverable and verifiable. Architecture changes MUST be a dedicated hop, not bundled with a language hop.

Example: `WordPress (PHP) → Symfony → NestJS → .NET → Rust → musl static binary`

Document the path in the Migration Speck (§13.11) and obtain user approval before implementation.

### 13.8 UC-level task decomposition for migration (mandatory)

Migration tasks MUST be decomposed per §12.6.1 — down to individual use cases. For each UC in scope, create separate sub-tasks per layer (BE, FE, BE/FE integration) as applicable. Multiple sub-tasks per UC are expected for complex UCs.

No migration work begins on any UC until:
1. Documentation for that UC is complete (§2.3 — functional + BE tech + FE tech requirements)
2. All UC-level sub-tasks for that UC are estimated `LOW`
3. All missing dependency rewrites for that UC are at Phase 1 parity (§13.6)

### 13.9 Migration order within a hop (mandatory)

1. **Backend (API) — class by class:** one class/module/service/controller/repository at a time; 100% behavioral parity per unit before starting the next
2. **Frontend (UI) — class by class:** only after backend migration is complete and verified; same discipline

### 13.10 Parity verification per hop (mandatory)

Two stages, matching §13.3:

**During migration — curl parity (per unit):** after each class/unit is migrated, run the curl suite. Every endpoint covered by that unit MUST produce byte-for-byte equivalent response shapes, status codes, and error messages. Do not proceed to the next unit until curl parity is confirmed.

**At hop completion — full parity (curl + Playwright/TUI):** once all UC sub-tasks are done and target unit/integration tests pass:
- Full curl suite (positive + negative + security) against both source and output — identical responses
- **Web UI:** full Playwright suite (all UCs, all devices §7.3) against both source and output — identical screen states, flows, and baselines
- **TUI:** full color screenshot suite (all UCs, §13.3.4) against both source and output — identical character-cell-level renders and render order

Any divergence = migration defect; resolve before declaring the hop done and starting the next hop.

### 13.11 Migration Speck (mandatory)

Follows §3 and §12. Additionally MUST include:
- **Feasibility assessment** — direct or stepping-stone, with justification
- **Stepping-stone path** (if applicable) — ordered hops with technology at each node
- **Phase breakdown per hop**
- **Class/unit inventory table** per hop: source unit → target unit, status (`PENDING` / `IN PROGRESS` / `DONE`)
- **UC decomposition table** per hop: `UC-###` → BE sub-task / FE sub-task / INT sub-task IDs + status
- **Dependency cascade inventory** per hop: dependency name → classification (available / replaceable / missing) → rewrite sub-task ID if missing
- **Pre-migration curl task reference** — backlog task ID for curl suite written against source (§13.3.1), status
- Deobfuscation status (N/A if not applicable)
- Explicit ACs for parity: curl suite passes on both source and output (per unit and at hop completion); Playwright suite (web UI) or color screenshot suite (TUI) passes on both at hop completion (§13.10)
- **TUI baseline details** (if TUI): terminal dimensions, screenshot tool + capture command, baseline storage path

### 13.12 Pre-migration: deobfuscation (mandatory)

If any source is obfuscated or minified, reconstruct it to human-readable state before migration:
1. Identify all obfuscated/minified files
2. Apply appropriate tooling
3. Manually restore meaningful names where tooling cannot (document assumptions)
4. Commit as a distinct step before any migration work
5. User reviews and accepts before migration proceeds

### 13.13 Forbidden migration actions (hard)

- Starting migration without complete, user-accepted `docs/spec.md` (§13.2)
- Migrating any UC whose §2.3 spec section (functional + BE tech + FE tech) is incomplete
- Starting any hop without a passing, complete curl test suite against the hop's source (§13.3.1)
- Declaring a hop done without the full Playwright UI parity suite passing against both source and output (§13.3.2)
- Declaring a TUI hop done without character-cell-level color screenshot parity and identical render order (§13.3.4)
- Direct migration assessed as infeasible
- Starting any hop without full `LOW`-estimated sub-task decomposition
- Starting migration on obfuscated/minified code without completed deobfuscation
- Substituting, skipping, or dropping a missing dependency instead of rewriting it in cascade (§13.6)
- Starting a UC sub-task before its missing dependency rewrites reach Phase 1 parity
- Migrating frontend before backend is fully verified within a hop
- Migrating more than one class/unit per step
- Altering API contracts, renaming fields, reordering response keys, or rewording error messages during Phase 1 or 2
- Mixing Phase 1/2/3 work in the same step
- Architecture changes inside a language-migration hop
- Declaring a hop done without both source and output passing full curl + UI suites with identical outcomes
- Starting next hop before current hop is declared done

---

## Appendix A — Minimal Use Case Template (`docs/spec.md`)

```markdown
### UC-###

**Summary:** one sentence.
**Description:** actors, trigger, outcome.
**Related BR/WF:** BR-###, WF-###

#### Functional Requirements
- Acceptance conditions (user-observable outcomes)
- Input/output data
- Validation rules, constraints, authorization
- Failure modes: validation / auth / empty / conflict (user-visible messages)

#### Technical Requirements — Backend
- Endpoints: `METHOD /path` → request shape → response shape → status codes
- Data model: entities, fields, relations, migrations
- Service logic: business rules, validations, error handling
- Security: auth, RBAC, rate limiting

#### Technical Requirements — Frontend
- Screens / components involved
- Interactions and navigation
- State management and data binding
- API calls (reference backend endpoints)
- Responsive behavior, accessibility

#### Flow (Mermaid)
flowchart TD
  A[Start] --> B[Step] --> C[End]

#### Schematic layouts
- docs/ui-schematics/UC-###/01-list.txt
- docs/ui-schematics/UC-###/02-detail.txt

#### Baseline screenshots
- docs/ui-baselines/UC-###/<project>/01-start.png
- docs/ui-baselines/UC-###/<project>/02-filled.png
- docs/ui-baselines/UC-###/<project>/03-success.png
- docs/ui-baselines/UC-###/<project>/04-error-validation.png

**Playwright video:** at least one UI test with recorded video MUST cover this UC.
```

---

## Appendix B — Minimal Speck Template (`docs/specks/SDD-###-*.md`)

```markdown
# SDD-### — Title

**Impacted UCs:** UC-###
**Impacted BR/WF:** BR-### / WF-###

## Scope / Non-goals
...

## Acceptance Criteria
- AC-1: ...

## Security Acceptance Criteria
- SEC-1: (e.g., injection payloads rejected with 400)
- SEC-2: (e.g., IDOR returns 403/404)
- SEC-3: (e.g., no memory leak under repeated load)

## Failure Modes / Error Mapping
...

## Test Matrix

| AC    | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
|-------|------|-------------|----------|---------|----|-----------|------------|
| AC-1  | ✅   | ✅          | ✅       | ✅      | ✅ | ✅        | ✅         |
| SEC-1 | ✅   | ✅          | ✅       | ⬜      | ✅ | ✅        | ✅         |

✅ = meaningful assertions (status + body + outcome; or UI interaction + baseline compare).
Every AC: positive + negative cases. SEC rows: security/adversarial vectors per stage.
```

---

## Appendix C — Minimal Backlog Task Template (`docs/backlog/TASK-###-*.md`)

```markdown
# TASK-### — Title

**Status:** INTAKE | REFINED | REFINED:BLOCKED | ESTIMATED | DECOMPOSED | READY | IN PROGRESS | DONE | REJECTED | DUPLICATE
**Created:** YYYY-MM-DD  **Updated:** YYYY-MM-DD

## Raw Request
> (Verbatim — do NOT edit after intake)

## Refined Description
**Scope:** ...  **Non-goals:** ...
**Impacted UCs:** UC-### (or "new UC required")
**Impacted BR/WF:** BR-### / WF-###
**Dependencies:** TASK-### (if any)
**Risks / Open Questions:** ...

## Estimation
**Level:** VERY LOW | LOW | MEDIUM | HIGH | IMPOSSIBLE TO ESTIMATE
**Justification:** ...

## Sub-tasks (if DECOMPOSED)

| Sub-task ID             | Title | Estimation | Status | Depends On |
|-------------------------|-------|------------|--------|------------|
| TASK-###-A-short-title  | ...   | LOW        | DONE   | —          |
| TASK-###-B-short-title  | ...   | LOW        | READY  | TASK-###-A |

## Execution Order
1. TASK-###-A
2. TASK-###-B (depends on A)

## Speck Reference
docs/specks/SDD-###-short-title.md
```
