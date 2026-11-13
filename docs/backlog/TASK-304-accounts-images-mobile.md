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
