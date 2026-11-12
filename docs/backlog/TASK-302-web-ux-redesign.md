# TASK-302 — Web UX/UI audit, redesign and mobile web (before the Android app)

Status: DONE on DEV (2026-11-12) — awaiting the owner's look; prod only on his command
Lane: PROJECT (rules.md §0: >1 day, new direction for the UI) · brief mode: pro (direction given in full; the
look-and-feel is built and shown, not asked — rules §9) · parent: TASK-300

## Request (owner, 2026-11-12, verbatim)

> Е, Пройдися в браузері по всім сторінкам, по всьому UX, проаналізуй UX, проаналізуй UI, сформуй список
> покращень, які ти би змінив, щоб UI був сучасний. Можливо, там кольорову палітру змінити, там форми кнопок,
> розположення кнопок, полів, по функціоналу подивися. Тобто треба такі от якби продукт аналіз провести по
> дизайну, щоб зрозуміти, які у нас речі добре зроблені, які погано, що треба виправити, як саме. І запропонує
> оновлений флоу для юзера, для організатора і просто того, хто купує квитки. Оновлений дизайн має бути. Ну,
> типу, що треба оновити в дизайні, покращити. І про ну, типу, і сформує план покращення, реалізуй його, потім
> протестуй теж в браузері, чи все окей, потім даси мені на деві переглянути результат. Тобто ідея така. Зараз
> робимо веб-частину, щоб була класно зроблена UI UX, і ми не втратили нічого там по функціоналу. Зробили
> хороший зручний функціонал. І потім на основі цього, якщо там все буде добре, зробимо мобільний додаток. І
> додамо плашку про те, що можна скачати додаток з QR-кодом на скачування. Поки що будемо його вантажити з
> сервера, поки немає у нас додатку на Google Play. Також має бути адекватна робоча мобільна версія, щоб було
> зручно і можна було користуватися у вебі з телефона цим. От, будь ласка, виконай цю задачу. Дуже важливо, щоб
> у нас продукт був якісним. Тобто VP має бути простою, як ми й задумали, але при цьому якісною, зручною і
> красивою по дизайну.

## Readings recorded

- Order change: web UX/UI now comes before the Android work (plan steps 2 and 4 reordered by the owner). The
  MVP's web-side product changes from step 1 that shape the flows (free-only UI, 1–5 places, guest list without
  email) are part of the new flows, so they land here; the mail service still waits for the owner's key.
- "Не втратили нічого по функціоналу": every route that works today keeps working; payment-only screens are
  hidden behind the free-only mode, not deleted (brief: payment code stays in its branch).
- App banner: the QR points to an APK download page on the server; the APK itself ships in the Android step.
- Visual direction: the brief's Figma direction (dark theme, blue accents, UNDERGROUND wordmark); the Figma file
  is not reachable from this session, so the redesign follows that direction and the owner corrects on dev.

## Owner approvals (2026-11-12, in chat, verbatim answers)

- Test assertion change `tickets_reserve.rs::free_ticket_deduped_per_user` (second reserve 409 → 201, up to 5
  places): «Так, погоджую».
- Three more assertion changes forced by the same decisions — `staff.rs` accept 204 → 200 `{event_id}`,
  `web_cabinets.rs` no `owner_email` in the guest list, `eventsocial.rs` friends = mutual follows: «Так, усі три».

## Implementer assumptions — triage (orchestrator)

| Assumption | Verdict |
|---|---|
| Recipient cap (CT4) only on free events (paid checkout has no per-person cap) | accept — MVP is free-only |
| A previous owner's QR answers `not_found` | accept |
| QRs minted before the deploy fail until refreshed (`oid` required) | accept — the web/app refresh every 45 s |
| Invites refused on blocked events too | accept |
| Staff list exposes `user_id` and invite `id` (needed for remove/revoke) | accept — organiser only, no email |
| P16 returns only free active tickets; nothing on admin block | accept for MVP; admin block → follow-up |
| P13 prune per bucket + whole table ~1/64 calls | accept |
| Rate limits env-configurable | accept |
| `meets_age` generic over the executor | accept |
| P2 read as `admin_locked` (organiser keeps own hide/publish; cannot undo moderation/cancellation) | accept — better than my wording, keeps an existing test |
| CT5 cancel-all not built | accept — the web cancels per ticket through the existing return endpoint |
| Local `notifications::city_match…` fails on Postgres 14 `lc_ctype=C`, same at base | to verify in CI (postgres:16) |

## Web review 1 (distinct, opus/xhigh, of ab835f6) — triage

| # | Sev | Finding | Decision |
|---|---|---|---|
| B1 | BLOCKER | event "over" at its start: late guests lose the QR, staff lose the scanner | fixed: `isOver` = `ends_at` or start + 12 h, used by tickets, dashboard, manage page, event page; booking still closes at the start ("already under way"); API adds `event_ends_at` to tickets |
| M1 | MAJOR | open redirect via `safeNext` ("/\t/evil") | fixed: `lib/safe-next.mjs` (control chars, backslash, "//", parsed-origin check) + node:test, mutation-proven |
| M2 | MAJOR | staff invite token copied into `/login?next=` | fixed: token waits in sessionStorage, fragment cleared, `?token=` fallback dropped |
| M3 | MAJOR | `node --test lib/` red on Node 22 (glob) | fixed: `node --test lib/*.test.mjs` |
| M4 | MAJOR | `javascript:` href sink (links, video) | fixed client-side (`safeHref`, gallery filter); API scheme validation in the API fix batch |
| M5 | MAJOR | APK / gift page show times in a different convention | gift page fixed; APK publish held behind `UG_PUBLISH_APK=1` until the Android step fixes times (dev data has no legacy rows to backfill; no prod) |
| 1 | MINOR | cap counted on `active` only | fixed: active + pending_claim + used |
| 2 | MINOR | fake "Готово" banner from URL | fixed: count from data, params cleared on arrival |
| 3–4 | MINOR | camera stream leak on double tap / tab leave; red flash after admit | fixed: start guard + unmount check; admitted codes remembered 120 s |
| 5 | MINOR | any `getMe` failure signs out | fixed: only 401 |
| 6 | MINOR | modal re-focuses on every render, no trap | fixed: effect once, Tab trap, focus restore |
| 7, 20 | MINOR | inbox re-selects deep-linked peer, stale thread; `?to=` unencoded into API path | fixed: applied once + URL cleared; messages cleared on switch; uuid check |
| 8 | MINOR | staff have no way back to the scanner | fixed: "You scan tickets at these events" on My events |
| 9 | MINOR | banner claimed offline + notifications | fixed: copy claims only what exists |
| 10 | MINOR | debug-signed APKs may not update over each other | moot while the APK is held; stable keystore + versionCode in the Android step |
| 11 | MINOR | paid events shown as free | API fix batch: free-only coercion on create/update |
| 12 | MINOR | /terms and /privacy still describe escrow/KYC | owner: legal text approval (plan step 6) |
| 13 | MINOR | guest CSV export removed | fixed: restored, `lib/csv.mjs` (RFC 4180 + formula guard) + node:test, mutation-proven |
| 14 | MINOR | mobile booking bar covers footer | fixed: extra footer padding when the bar exists |
| 15 | MINOR | `--muted-2` contrast 3.8–4.1:1 | fixed: #8a8fa1 |
| 16 | MINOR | date filter in visitor's zone vs card in event's zone | fixed: filter in the event's zone |
| 17 | MINOR | remembered city with no events filters to nothing | fixed: dropped when absent |
| 18 | MINOR | mobile chat compose under the tab bar | fixed: height allowance |
| 19 | MINOR | no end time in the form | fixed: optional "Кінець", validated after start |
| T | TRIVIAL | decodeURIComponent throw, loading treated as signed out, missing labels, deploy APK messages/atomic manifest | fixed |

## API fix batch 1 (32621df) — implementer assumptions, triage

| Assumption | Verdict |
|---|---|
| Free-only is a process-wide switch from `FREE_EVENTS_ONLY` (default on); in-process tests run with it off except the 11c test | accept — the deployed binary runs `main`; closure check confirms the default |
| Update only zeroes a sent price; existing paid events are not converted | accept — DEV demo data is re-seeded free; no prod |
| Requests without X-Forwarded-For or peer share one "unknown" bucket (tests only) | accept |
| 10 sign-ups + code requests per IP per hour may be tight behind venue NAT | accept for MVP; env-tunable; revisit with traffic |
| 0048 renames only password-less accounts named exactly their email local part | accept — password accounts typed a name; CT8 fallback still hides it from organisers/staff |
| Item 4 leaves paid tickets and gifts alone | accept — free-only |
| Item-3 timing test relies on argon2 ≥ 3× a DB lookup (≈100× locally) | accept; watch CI for flakiness |
| `task302_controls::p9_…` assertion changed (hidden event: 200 empty → 404) | test written inside this task, not a pre-existing test; no owner approval needed |
