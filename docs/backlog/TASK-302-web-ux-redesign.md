# TASK-302 — Web UX/UI audit, redesign and mobile web (before the Android app)

Status: IN PROGRESS (2026-11-12)
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
