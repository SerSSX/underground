# TASK-201 — Luma-режим: швидке створення + post-publish share (INIT-QUICKCREATE leaf-1)

**Status:** DONE (verified: Android device + Web browser)  **Parent:** TASK-199-C  **Phase:** 2
**Estimation:** LOW  **Created:** 2026-07-02  **Done:** 2026-07-02

> **Реалізовано + перевірено (FE-only, без backend/spec-змін):**
> - **Дата опціональна:** прибрано `dateMillis != null` з `canSubmit` (Android); web-форма вже без `required` (`toIso("")→null`).
> - **Post-publish share:** `CreateEventViewModel.submit` тепер віддає `created: EventDto`; Android `CreateEventScreen` показує
>   діалог «🎉 Опубліковано!» + лінк `${WEB_BASE_URL}/events/:id` + системний share-sheet; web `organizer/new` — екран
>   «🎉 Опубліковано!» + Скопіювати/Telegram/WhatsApp + Відкрити подію/Мої події.
> **Android (device):** створено «Quick» БЕЗ дати → **DB: published, starts_at=NULL, free** → share-діалог → share-sheet
> «Приходь на «Quick»: <link>» (Telegram/Gmail). **Web (browser):** «Web Quick Rave» БЕЗ дати → **DB: published, starts_at=NULL** →
> share-екран із лінком, 0 console-помилок. Android compile+unit ✓.

## Raw Request
> «Anyone creates events» + viral. Зробити створення події швидким (Luma-mode) і замкнути віральний хук:
> ОДРАЗУ після публікації — екран «Готово! Поділись», щоб організатор поширив лінк події в месенджери.

## Refined Description
**Scope (FE-UX only — API вже підтримує; без spec/behavior-змін):**
- **Дата опціональна:** прибрати обов'язковість дати у формі створення (Android+Web). API `CreateEventReq.starts_at`
  вже `Option`; подія публікується без фіксованої дати (TBA). Дефолти вже є (категорія, місто=Київ, квитки=50).
- **Post-publish share (віральний хук):** `submit` повертає створену подію (`api.create` вже віддає `EventDto`);
  замість миттєвого `onDone()` — показати «🎉 Опубліковано! Поділись, щоб прийшли» + лінк `${WEB_BASE_URL}/events/:id`
  + системний share-sheet (Telegram/…); той самий формат лінка, що `EventDetailScreen.share()`.

**Non-goals:** окремий «one-tap» шаблонний майстер; KYC JIT (TASK-199-C інше листя); зміна create-API/схеми; draft-lifecycle.

**Impacted:** UC-Create (UX-варіант; поведінка незмінна — дата вже optional у контракті). Без нових BR/міграцій/spec-змін.

## Estimation
**Level:** LOW — суто клієнтський UX поверх наявних `POST /api/events` (повертає подію) + `starts_at` nullable.
Немає нового endpoint/схеми/грошей. Верифікація: створити подію без дати на пристрої+браузері → share-екран → лінк.
