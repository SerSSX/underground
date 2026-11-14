# TASK-307 — no ticket offer to one's own email

Owner, 2026-11-14 (voice, verbatim):
> Якщо я сам собі можу надіслати подарунок, але не отримую жодних повідомлень про це ні в додатку, ні інакшим чином,
> то це, мабуть, бага. Тобто, якщо сам собі надсилаєш, то варто би, мабуть, заборонити таку дію, щоб людина не могла
> надіслати сама собі цей квиток, якщо і так у неї. Треба це пофіксувати. В остальному виглядає все ніби як нормально.

Triage: TASK (a refusal in the gift/transfer endpoints; tickets.rs is SENSITIVE by the §1 grep). Brief mode: pro.

## Change
- `POST /api/tickets/:id/gift` and `POST /api/tickets/:id/transfer`: after the ownership check, an email equal to the
  caller's own (trimmed, lower-case) → 422 «квиток уже ваш — надішліть його іншій людині»; nothing is written.
- Reverses part of TASK-305 R2 ("also for ... self: accepted"): refusing the caller's OWN address reveals nothing about
  any other account, so the account-oracle argument does not apply to it. Every other valid address still gets 202.
- Web shows the refusal through `errorMessage` in all 9 languages (`web/lib/server-messages.mjs`, `web/locales`).
- Android shows the server text as sent; checked in the Android step.
