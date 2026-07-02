# SDD-118 — Делегований door-scan + безпечний signed QR + вердикти (STAFF-1a)

**Impacted UCs:** UC-007 (Сканування QR на вході — new UC required, see TASK-101 spec reconciliation), UC-005 (повернення → вердикт refunded)
**Impacted BR/WF:** BR-010, BR-012 (розширення на делегованого `event_staff` з perm=scan + atomic anti-passback — spec-first у TASK-101), WF-Escrow (door-flow НЕ рухає гроші; `api/src/escrow.rs` недоторканий), новий `event.status='cancelled'`

## Scope / Non-goals
**Scope:** таблиця `event_staff` (event_id, user_id, perms=['scan'], invited_by) + серверний guard
`require_perm(event_id, user, perm)`; event-scoped `POST /api/events/:event_id/tickets/validate` з
атомарним first-write-wins anti-passback і повним набором вердиктів; signed stable QR token (HS256,
seam `api/src/auth.rs`; claims tid/eid/exp, short-TTL ~120s) поверх стабільного `ticket_id` — сирий
`qr_token` не покидає сервер у UI-відповідях; інвайт-лінк «Контролер» на тому ж signed-token seam
(preview ДО логіну, accept у 1 клік, guest-signup); Android door mode (сканер+вердикти, показ signed
QR, запросити/прийняти контролера, «Мої події як контролер»); `event.status='cancelled'`.
**Non-goals:** STAFF-1b continuous Door Mode UI; STAFF-1c offline-черга; STAFF-1d scan audit-лог;
TOTP/rotating-QR; refund/escrow на дверях (прибрані); ролі понад `scan`; веб-адмін керування staff;
web-клієнт (лише API `api/src/tickets.rs`/новий `api/src/staff.rs` + Android `android/.../ui/door/**`).

## Acceptance Criteria
- AC-1: Organizer події створює інвайт-лінк контролера; перехід+accept створює рядок `event_staff` з
  perms=['scan']; після цього контролер успішно сканує квитки САМЕ цієї події.
- AC-2: Контролер БЕЗ прав на іншу подію отримує 403 при `validate` квитка чужої події (`require_perm`).
- AC-3: Перший scan активного квитка → verdict=`valid`, атомарно `status active→used`, `scanned_at`/
  `scanned_by` заповнюються.
- AC-4: Повторний scan того самого QR (у т.ч. збереженого/реплейнутого офлайн-клієнтом) → `already_used`,
  вдруге НЕ впускає (first-write-wins, без гонок при паралельних скан-запитах).
- AC-5: Scan квитка, чий `eid` ≠ `:event_id` у path → `wrong_event`, статус НЕ змінюється.
- AC-6: Кожен вердикт `valid|already_used|wrong_event|refunded|expired|not_started|cancelled|not_found`
  повертається з машинним `code` і рендериться з унікальним кольором+іконкою+текстом (non-color-redundant).
- AC-7: Прострочений signed QR (exp минув) → `expired`; квиток лишається `active` і придатний після
  повторної видачі власником.
- AC-8: У застосунку НІДЕ не показується і не логується сирий `qr_token`; фігурує лише signed token.
- AC-9: Гість без акаунта за staff-інвайтом бачить назву події+організатора ДО логіну, реєструється в
  пару кліків і одразу стає контролером (auth-wall лише на accept).
- AC-10: На door-flow недоступні жодні refund/escrow-операції (немає ендпоінтів/кнопок).
- AC-11: staff-invite токен single-use і short-TTL: повторний accept тим самим токеном або після TTL
  відхиляється (4xx).
- AC-12: Organizer бачить список контролерів події і відкликає будь-кого (DELETE); після відкликання
  його `validate` → 403.
- AC-13: Подія зі `status='cancelled'` → вердикт `cancelled` для всіх її квитків при скані.

## Security Acceptance Criteria
- SEC-1 (anti-passback/double-entry): атомарний first-write-wins `UPDATE tickets SET status='used',
  scanned_at=now(), scanned_by=$u WHERE id=$tid AND status='active' RETURNING` у транзакції гарантує
  рівно один `valid` навіть при паралельних/офлайн-реплей запитах; `scanned_at` НЕ є ключем ідемпотентності.
- SEC-2 (secret leak): у UI показується короткоживучий signed token, а не сирий `qr_token`; сирий токен
  не присутній у жодній відповіді, яку споживає UI сканера/власника (контрактний тест на форму відповіді).
- SEC-3 (authz/IDOR privilege-escalation): `require_perm` суворо перевіряє `organizer_id` ПОДІЇ або
  активний `event_staff` з perm=scan САМЕ для цього `event_id`; глобального scan-права немає.
- SEC-4 (cross-event replay): `eid` всередині signed token + event-scoped ендпоінт унеможливлюють
  використання валідного квитка однієї події на дверях іншої → `wrong_event`.
- SEC-5 (token forgery/algorithm-substitution): HS256 pinned на encode+decode (як `auth.rs`), інші alg
  відхиляються; невалідний підпис → `not_found`/`expired`, НІКОЛИ не `valid`.
- SEC-6 (invite abuse/rate-limit/oversized): single-use + short-TTL + rate-limit на `/staff/invite` та
  `/staff/invite/:token/accept`; device/email-cap на guest-signup з інвайту; oversized `qr_token`/body
  відхиляється до декодування.
- SEC-7 (replay after revoke): відкликаний контролер (DELETE `event_staff`) миттєво втрачає доступ —
  `require_perm` читає актуальний рядок, кешованих прав немає.
- SEC-8 (least-privilege / refund-at-door removed): контролер не може повернути/заморозити гроші через
  door API; escrow seam `api/src/escrow.rs` недосяжний з door-flow.
- SEC-9 (injection): `qr_token`/`:event_id`/`:token` параметри — параметризовані запити SQLx; жодної
  конкатенації; невалідний UUID/JWT → безпечна 4xx без розкриття.

## Failure Modes / Error Mapping
- Невалідний/підроблений/чужий-alg signed QR → `200 {valid:false, verdict:"not_found"}` (не 500, не leak).
- Прострочений QR → `200 {valid:false, verdict:"expired"}`; квиток лишається `active`.
- Квиток іншої події → `200 {valid:false, verdict:"wrong_event"}`; статус незмінний.
- Повторний скан → `200 {valid:false, verdict:"already_used", scanned_at}`.
- Квиток `returned/refunded` → `verdict:"refunded"`; `event.status='cancelled'` → `verdict:"cancelled"`;
  `now < starts_at/doors_at` → `verdict:"not_started"`.
- Caller без perm на подію → `403`; не-organizer на `/staff/invite`|`/staff` → `403`.
- Прострочений/використаний/невалідний staff-invite токен → `410`/`400`; rate-limit перевищено → `429`.
- Oversized body/token → `413`/`400` до JWT-декодування.

## Test Matrix
| AC | Unit | Integration | Curl Dev | Base UI | UI | Curl Prod | Prod Stack |
| --- | --- | --- | --- | --- | --- | --- | --- |
| AC-1 invite→accept→staff scan | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-2 cross-event 403 | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-3 valid→used atomic | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-4 already_used replay | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-5 wrong_event no-mutate | ✅ | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ |
| AC-6 verdict rendering | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ⬜ |
| AC-7 expired QR re-issue | ✅ | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ |
| AC-8 no raw qr_token in UI | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ⬜ |
| AC-9 guest invite pre-auth | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ | ✅ |
| AC-10 no refund at door | ✅ | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ |
| AC-11 invite single-use/TTL | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| AC-12 staff list/revoke | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| AC-13 cancelled verdict | ✅ | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ |
| SEC-1 anti-passback race | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-2 no secret leak (contract) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| SEC-3 authz privilege-escalation | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-4 cross-event token replay | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-5 alg-substitution/forgery | ✅ | ✅ | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| SEC-6 invite abuse/rate-limit/oversized | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-7 replay after revoke | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
| SEC-8 refund-at-door removed | ✅ | ✅ | ✅ | ⬜ | ✅ | ✅ | ✅ |
| SEC-9 injection (sql/param) | ✅ | ✅ | ✅ | ⬜ | ⬜ | ✅ | ✅ |
