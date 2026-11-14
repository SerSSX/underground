# TASK-305 — DEV feedback: chat input dead, organiser page, mobile zoom, multi-account QA

Status: IN PROGRESS (2026-11-13) · Lane: TASK · parent: TASK-300

## Request (owner, 2026-11-13, verbatim, voice transcript)

> Так, перше, коли я заходжу тепер в Messages, намагаюся тапнути хоч кудись оце в поле для вводу, нічого не
> відбувається. Тобто воно має показувати тобі от переписку, яка там є. Якщо її немає, то він там ну, не показувати
> її, да? Має бути поле для вводу, і коли я ввожу туди, має ну, писатись повідомлення в цьому полі. Натискну кнопку
> відправити, там має бути якийсь типу самольотик чи щось таке, типу, ну, типу, як зрозумілий символ для відправки з
> повідомлень. Е, Воно має з'явитися в мене на інтерфейсі, ну тобто, і як би да, відправитися там далі на сервер.
> Тобто зараз я бачу, що нічого не працює. Тобто воно не працює. Е, Зараз ще перевіримо в інкогніто. На ринку то теж
> не працює. Тобто, ну тут, скоріше все, проблема не в тому, що щось не підгрузилося або там закешувалося, тому що на
> веб-версії, яку я відкриваю, вона теж не працює. Потім у нас на сторінці оцього от події є така штука, як
> організатор. Там показується блок, але я не можу клікнути на організатора. Треба, щоб можна було побачити всі
> його, ну, типу, його сторінку, щоб кожен міг перейти і подивитися. [...] На мобілці все одно, ну, можна зумити
> його якось, ну, може це, мабуть, браузер дає це робити, я не знаю. Це не дуже зручно. Хотів би, щоб цього не було,
> якщо це можливо, звісно. [...] Відновлення пароля заработає тільки на ресенді, я так розумію, так? [...] Дивись,
> ще потестуй. Зроблю декілька різних аккаунтів, поімітуй процеси. Ну, типу, типові юзер-процеси [...] Попробую якісь
> нетипові, щоб ми там перекрили якісь там кейси нестандартні [...] на якісь аб'юзи і все остальне. [...] по безпеці
> треба бути чек зробити. [...] пройдися з різних аккаунтів, по взаємодії, тобто таке тест проведи, подивися,
> скільки це все зручно, чи всі функції працюють точно, може навіть щось ще по UX, оце от з чатом треба, плюс
> подивися все, що де може здатися юзеру, щоб можна все тиснути і перейти. Якщо це можна таке додати туди, то давай
> будемо додавати. [...] завтра вирішимо проблеми з розсилками, мейлінгом [...] потім мобайл-версія [...] релізити.

(Full transcript in the session; elided parts are the owner's walkthrough of what already works: event management,
tickets/QR, localization, address search, password change.)

## Items
1. Chat: the compose field does nothing on tap (mobile, incognito, desktop web). Reproduce, root-cause, fix; a clear
   send icon (paper plane — already `Send` from lucide; check it renders).
2. Organiser block on the event page is not clickable → a public organiser page (profile + their events).
3. Mobile zoom still possible (iOS Safari ignores `user-scalable=no`) → block pinch/double-tap by script.
4. Multi-account QA on DEV: typical flows, atypical flows, abuse/security probes, UX "everything a user expects to
   tap leads somewhere"; fix what is found.

## Round 2 API (4d742af): assumptions triaged by the orchestrator, 2026-11-14

- R2 every transfer creates an offer, also for an unknown email or self: **accepted**. Doing nothing for an unknown email would show the sender `pending_claim` vs `active`, the same account oracle the owner's «Як подарунок» closes. A mistyped email holds the ticket until the sender cancels or the 48 h TTL ends, as for a gift.
- Claim token = HMAC-SHA256(jwt_secret, gift_id), only the hash stored: **accepted**. Rotating `jwt_secret` changes alert links of offers still pending; written into the runbook note in STATE.
- Backfill renames pre-2026-11-12 password accounts whose typed name equalled the email local part: **accepted** (DEV data only; prod has no such accounts yet).
- Avatar must match `PUBLIC_BASE_URL` exactly; stored external avatars untouched: **accepted**.
- Report reason allows newlines; malformed body with a NUL now 422 instead of 400: **accepted**.
- `discover.rs` MOMENTUM still counts every review: **accepted as tracked MINOR** (ranking only, nothing shown as a rating), due 2026-11-21.
- Push worker logged the device push token and the title: **rejected, fixed** in this round — it now logs `notification_id` and `kind` only.
- Telemetry limit is DB-backed (one `rate_events` row per call): **turned into a review question** for the round-2 reviewer (pruning of `rate_events`).
- Android: `POST /api/tickets/:id/transfer` now answers 202 `{"status":"sent"}`; `EventApi.kt:200` expects `TicketDto`. **Blocks the next APK** (the APK is held anyway); recorded in STATE.
- Web gap found in triage: alerts opened only `/events/<id>`, so a place offer could not be accepted from the web. **Fixed**: `web/lib/alert-href.mjs` follows the alert's own `link` when it is a plain path on this site.

## Round 3 (a770792): reviewer NO-GO B1-B6 fixed; assumptions triaged 2026-11-14

- Anonymous telemetry cap counted per UTC day, check-then-insert not atomic (may slightly overshoot): **accepted** — a soft cap on a disk-fill guard; overshoot is bounded by the per-IP limit.
- Retention clears at most 10 s per hourly tick: **accepted**.
- Transfer while an offer is still open answers 404 where gift answers 409: **accepted** (only the held Android build calls transfer); noted for the Android update.
- Spec corrected: a guest's `user_id` goes to the event's organiser only (code is stricter than the round-2 spec text).
- Open MINOR after this round: `discover.rs` MOMENTUM counts every review, due 2026-11-21.
