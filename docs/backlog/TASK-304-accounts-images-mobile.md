# TASK-304 — Password change/reset, "account exists → log in", WebP images, mobile chat + zoom

Status: IN PROGRESS (2026-11-13) · Lane: TASK (sensitive: auth, uploads) · parent: TASK-300

## Request (owner, 2026-11-13, verbatim)

> 1) Треба функціонал для зміни пароля
> 2) Треба функціонал відновлення пароля
> 3) Якщо акаунт уже є то пропонуємо увійти і даємо кнопку для цього
> 4) Помітив що великі фото подій грузяться довго, треба додати стиснення і конвертацію в WebP що би ми швидко вантажили контент.
> 5) також коли з моб версії пишеш повідомлення поле для вводу пропадає. Треба пофіксити.
> 6) Також треба на моб адаптиві фіксувати розміри екрану і не давати розтягувати чи зумити сторінку.

## Owner answer

- Mail service for password reset (and login codes): «Resend з ssvstudio.dev» — the owner creates the Resend
  account, verifies the domain with DNS records at Porkbun and creates the key; an agent never sees the key.

## Also in this batch

TASK-303 closure MINORs N1–N4 (seed onboarded flag, city whitespace compare, tzf memory check, trustedCityRow).

## Build notes (2026-11-13, implementer)

### API
- `POST /api/me/password {current?, new}` → 200 `{token}` (not 204: every older token dies, so the caller gets a
  fresh one for this device). Password accounts: `current` required, wrong → 422 "неправильний поточний пароль"
  (not 401, so the web never reads it as "signed out"). Password-less accounts may SET one only within 10 min of
  sign-in (`iat`), else 401 "увійдіть ще раз, щоб задати пароль". 8–256 bytes. 10 tries/hour/account
  (`PWCHANGE_LIMIT_PER_USER`). Sessions: the existing `users.tokens_valid_after` watermark (checked on every
  authenticated request) is set to `date_trunc('second', now()) + 1 s` and the new token is minted with `iat` = that
  second, so even a token from the same second dies (JWT `iat` has 1 s resolution). Open reset links are retired.
- `GET /api/me` (and every `User`) carries `has_password` (never the hash).
- Reset: tokens were already single-use and SHA-256-hashed; TTL now 30 min (was 60); asking again retires the
  older link; `forgot-password` limits 20/h per IP and 5/h per address, both BEFORE the lookup (so a 429 is no oracle
  either); response always 204 with an empty body; mail sent from a background task (answer timing does not depend
  on Resend). `reset-password` 30/h per IP.
- Mail (`api/src/mail.rs`): Resend over HTTPS, 5 s timeout, no redirects, no proxy, `Idempotency-Key` per mail, 2
  retries with backoff + jitter on 429/5xx/network. Logs: kind, status, Resend id — never key, recipient or body.
  Tokens/codes are never stored in plain text: there is no DB outbox for these mails, they live in memory until sent
  (a restart loses an in-flight mail; the user asks again). The order/notification outbox in `email.rs` is untouched
  (still log-only, SendGrid placeholder). Mail language: account locale `uk` → Ukrainian, any other → English; an
  address without an account (code sign-up) uses the request's `locale`. Text + HTML parts.
- `GET /api/features` → `{"email": bool}` (`Cache-Control: no-store`), true only when `RESEND_API_KEY` and
  `EMAIL_FROM` are set (and `RESEND_URL`, default `https://api.resend.com`, is https or loopback http).
- Uploads: decoded by `image` 0.25 (pure Rust: zune-jpeg, png, image-webp; MIT/Apache), format sniffed from the
  bytes (JPEG/PNG/WebP only), pixel count checked from the header before decoding (≤ 32 MP; decoder alloc cap 160 MiB,
  sides ≤ 20 000), scaled to ≤ 1600 px (never up), EXIF orientation applied, re-encoded as lossy WebP q80 by libwebp
  via `webp` 0.3 (crate MIT/Apache-2.0, libwebp-sys MIT, vendored libwebp BSD-3-Clause, includes the
  CVE-2023-4863 Huffman fix; libwebp only ever encodes our own pixels), plus `<id>-480.webp`. No metadata survives
  (only pixels are encoded). One image processed at a time. 120 uploads/hour/user (`UPLOAD_LIMIT_PER_HOUR`).
  Answer `{url, url_small}`. Older uploads untouched. Cap is 32 MP, not 40: the api container has 256 MB.
- TASK-303 N1 seed `onboarded = true` (both statements, insert and conflict update); N2 city compare = NFC
  (`unicode-normalization`, already in the tree via sqlx) + collapsed whitespace + lower case; N3 `EmbeddedFinder`
  (in-place over the ~4 MB data; a unit test compares it with `DefaultFinder` on a 0.5° world grid, 259 200 points,
  plus border points: identical); N4 web `trustedCityRow` returns only a row in the known country.

### Web
- `lib/features.ts` (`useFeatures`) replaces the build-time `NEXT_PUBLIC_EMAIL_LOGIN` flag: code sign-in (login,
  gift claim) and "Forgot?" appear only when the API says `email: true`; `/forgot-password` says reset by mail is not
  available when it is off.
- Sign-up with a registered address (409): "This email already has an account. Log in to it." + **Log in**, which
  opens `/login?next=…` with the address filled in. The address goes through sessionStorage, not the URL (PII stays
  out of history and server logs). The 409 itself is unchanged (pre-existing disclosure, rate-limited per IP/email).
- `/account` → Password: current + new + confirm (or "Set password" for code-only accounts); success stores the new
  token and says other devices were signed out.
- `/reset-password` takes the token out of the address bar on arrival.
- Pictures: `Poster` variants — cards `srcset` 480w/1600w with `sizes` (phones at 2–3× density still pick the 1600
  copy; 1× screens and every thumbnail use the 480 copy), thumbnails (tickets, organiser lists) the 480 copy, the event
  page the full picture; no copy (older uploads, links) → as before. Uploads over 1.5 MB or 1600 px are shrunk in the
  browser first (canvas → WebP, JPEG on Safari, PNG kept for transparency), which also lets 5+ MB phone photos through.
- Mobile chat: an open conversation is a fixed layer sized to `window.visualViewport` (shrinks with the keyboard),
  compose bar at its bottom, the page behind locked; the message list scrolls itself (the old `scrollIntoView` also
  scrolled the window). The send button keeps the keyboard open.
- Zoom (owner's request): `maximum-scale=1, user-scalable=no`. **Cost:** people with low vision cannot zoom (WCAG
  1.4.4); iOS Safari ignores `user-scalable=no` and still allows pinch. Fields are 16 px on phones (stops iOS
  focus-zoom), `overflow-x: clip` on html/body as a guard (no real overflow measured at 375 px on home, the event page,
  /account and the open chat: document width 375), `touch-action: manipulation` on controls, no horizontal overscroll.

### Owner steps for mail
See `deploy/README.md` → "Пошта (Resend)"; the key is set with `deploy/set-mail-key.sh` (owner only).

## Review fix batch (2026-11-13)

1. Reset now moves `tokens_valid_after` to the next whole second too (`date_trunc('second', now()) + 1 s`), as the
   change does; a JWT minted in the second of the reset is rejected (`task304_password::a_reset_kills_a_token_from_the_same_second`).
2. Upload memory is bounded from the header before decoding: `decode_cost` = decoded buffer (`total_bytes`, so 16-bit
   and alpha count) × a per-decoder multiplier, + the DCT coefficients of a progressive JPEG (parsed from SOF2), + a
   fixed 80 MiB for 16-bit PNG; refused above `DECODE_BUDGET` = 140 MiB ("too large"). `image`'s `max_alloc` is not
   applied by `from_decoder`, so it could not do this. Small pictures are scaled with the float filter only when 8-bit
   (16-bit took a 16 B/px float copy). Peak RSS, release build, one process (incl. its own ~6 MiB):

   | Picture | Before | Now |
   |---|---|---|
   | 16-bit RGBA PNG 5656² (32 MP, 249 KB file) | 311 MiB | refused, 6 MiB |
   | progressive 4:4:4 JPEG 32 MP | 307 MiB | refused, 8 MiB |
   | progressive 4:2:0 JPEG 32 MP | 214 MiB | refused, 7 MiB |
   | RGBA PNG 32 MP / lossy WebP+alpha 32 MP | 197 / 362 MiB | refused |
   | baseline JPEG 32 MP (4:4:4 or 4:2:0) | 124 MiB | 124 MiB, accepted |
   | RGB PNG 32 MP | 123 MiB | 123 MiB, accepted |
   | largest accepted: progressive 4:4:4 13.1 MP / RGBA PNG 19.1 MP / WebP 27.1 MP / WebP+alpha 8.6 MP / 16-bit RGBA PNG 3.8 MP / 16-bit RGB PNG 3.8 MP | — | 141 / 145 / 137 / 140 / 147 / 66 MiB |

   The pixel cap stays 32 MP; what limits a picture now is the memory it needs. Measured with the ignored test
   `task304_images::measure_decode_peak` under `/usr/bin/time -l`.
3. The decode permit is owned by the blocking task (`acquire_owned`), so a dropped request keeps its slot until its
   decode ends (`task304_upload_cancel`).
4. Mail budget: `MAIL_DAILY_BUDGET` (80) per rolling 24 h and 10 per address, counted in the DATABASE (`mail_sends`,
   address as SHA-256; survives restarts and replicas), taken before any account lookup for every address alike;
   spent → 503 "пошта тимчасово недоступна, спробуйте пізніше", address cap → 429; one WARN when the last slot goes.
   With mail off nothing is counted. Addresses must be one plain address (exactly one `@`, no `,;<>"`, no
   whitespace) at sign-up and code request; `mail::enqueue` refuses anything else as a last guard.
5. Timing: the cheaper fix — comments corrected (a reset writes a token row only for a registered address; existence is
   already disclosed by sign-up's 409).
6. Password bounds count characters. 7. The change is one transaction. 8–10: slow-Resend test, comments, redacting
   `Debug`, `grep` exit codes in `set-mail-key.sh`, and the chat follows the newest message on a keyboard change only
   for a reader within 80 px of it.

## Review round 3 (2026-11-13): a structural memory limit instead of the estimate

- The header estimate of item 2 above (`decode_cost`, `jpeg_coefficient_bytes`, the multipliers, `DECODE_BUDGET`) is
  **deleted**: it was bypassed twice (a decoy marker before a progressive frame; lossless and animated WebP far over
  their factor) and it refused valid JPEGs (a stray 0x00 between segments). The measurement table above describes
  the deleted model and is kept as history.
- Now every upload is decoded and encoded in a child process — the API binary started as
  `underground-api decode-image` (dispatched in `main` before Tokio starts; single-threaded; no environment, so no
  secrets; bytes on stdin, a length-prefixed frame on stdout). Limits set between fork and exec: `RLIMIT_AS`
  `IMAGE_DECODE_MEM_MB` (192 MiB; Linux only — macOS does not enforce it), `RLIMIT_CPU` 20 s, and a 30 s wall-clock
  timeout after which the API kills it. Any death of the child (allocation failure/abort, SIGXCPU, SIGKILL, a crash)
  answers 422 «зображення надто складне для обробки — збережіть як JPEG або зменште розмір» (all catalogs). One
  child at a time (owned permit, held by a task of its own until the child ends). The 32 MP header pixel cap stays as
  a cheap first gate, now with a true message (the decoder's own size limits are off, so nothing else says "32 MP").
  Animated WebP is refused with the same "too complex" message.
- Runtime image `debian:bookworm-slim`: the API runs as `appuser` and may fork/exec its own `/app/underground-api`;
  lowering its own rlimits needs no privilege.
- Test changes in this unit (was / now / why): `memory_heavy_pictures_under_the_pixel_cap_are_refused` (was: patched
  headers refused "too large" by the estimate) → `hostile_pictures_are_processed_or_refused_and_the_api_survives`
  (real progressive 32 MP JPEG with a decoy SOF3 frame header before its own, lossless 27 MP WebP, 16-bit RGBA 5656² PNG, animated WebP: stored or "too
  complex", and the next upload answers 201) plus the Linux-only `the_child_memory_limit_refuses_heavy_decodes`
  (each of the three heavy ones must be "too complex"); why: the estimate is gone. `jpeg_coefficient_estimate`
  deleted with its function. `one_address_gets_at_most_ten_mails_a_day` → `…five_mails_an_hour` (cap changed).
  `forgot_is_rate_limited_per_ip` moved from task304_mail to task304_password (mail off; with mail on the tighter
  per-IP mail cap answers first).
- Mail (E): `MAIL_DAILY_PER_IP` (10 per 24 h, IP hashed in `mail_sends.ip_hash`, migration 0051 edited — it is on
  no shared database yet) checked before the lookup for every address → 429; per-address cap now 5 per rolling hour.
  Residual in docs/decisions.md.
- Web: the /account password length counts code points (`[...s].length`), as the API does.

## Review round 4 (2026-11-13)

- F: a waiting room of `UPLOAD_QUEUE` = 3 uploads (one decoding, two waiting), taken with `try_acquire_owned` BEFORE
  the body is read; a 4th gets 503 «забагато завантажень одночасно, спробуйте за хвилину» (all catalogs). The place is
  held by the decode task until it ends. Bounds API memory to ~3 × 5 MB of bodies. Test `task304_upload_queue`.
  The size is `UPLOAD_QUEUE` (default 3, read once). The test binaries that upload from many parallel tests
  (task304_images, task304_upload_cancel, uploads, uploads_errors) set it to 64 in their setup — a fixture change, no
  assertion changed; `task304_upload_queue` keeps the production 3.
- G: the child stays outside the timed future; on timeout (and on an I/O error or an oversized answer) the API kills
  it and waits for it, so the decode slot is released only after the child has been reaped.
- I: `RLIMIT_CORE = 0` in the child: no core dump holding an uploader's picture.
- J: at most 16 + 4 + 2 × 16 MB read from the child's stdout, more → "too complex"; the child's encode failure is
  "too complex" (422), no longer 500.
- H and test changes (was / now / why):
  - `hostile_pictures_are_processed_or_refused_and_the_api_survives`: was — all four fixtures on every OS; now — the
    three ~300 MB ones on Linux only, the animated one everywhere; why — macOS does not enforce the child's limit, so
    on the owner's 8 GB Mac each really took ~300 MB (the Mac swapped to a full disk on 13 Nov).
  - New Linux-only binary `task304_image_limits_linux`: a 4032 × 3024 baseline JPEG answers 201 with the DEFAULT
    limit; the decoy JPEG 422 with the default limit and 201 with `IMAGE_DECODE_MEM_MB=4096` (a memory kill, not a
    time kill). It raises `IMAGE_DECODE_CPU_SECS` / `IMAGE_DECODE_WALL_SECS` to 300 for itself only, because debug
    decodes on a CI runner are slow; production stays at 20 s / 30 s.

## Before prod (record only, review round 4 K)

- Hash IPs and addresses in `mail_sends` with an HMAC under a server key, not plain SHA-256 (an IPv4 or a known
  address list can be brute-forced from a plain hash).
- Bucket IPv6 per-IP mail caps by /64 (one host owns a whole /64; today each address in it counts separately).
- People behind carrier-grade NAT share one IP and so share the 10-a-day mail cap.
