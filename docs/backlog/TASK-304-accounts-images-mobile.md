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
