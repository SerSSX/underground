# Feature decomposition — решта функціоналу (LOW-листя)

> Декомпозиція всього незробленого функціоналу у LOW-leaf таски (доктрина §12.5: HIGH/неестимоване → дробити до LOW перед кодом).
> Статуси: TODO / IN PROGRESS / DONE. Кожне листя при старті отримує власний `BLG/TASK-###` + speck за потреби.

## ✅ Зроблено (наскрізно працює)
auth (email/пароль), афіша+деталі, безкоштовний квиток + e-квиток QR, **платний квиток (test-режим) + ескроу held**,
створення івенту, QR-валідація на вході, веб-адмінка платежів.
2026-06-12: + пошук/фільтри API з тестами (F-17a), prefs+онбординг (F-10), профілі+підписки (F-11),
відгуки+рейтинг (F-13), сповіщення prefs-матч (F-14a/b).
2026-06-13 (соц-ітерація): таб «Стрічка» прибрано; **стіна профілю** (posts: ручні + автоанонс івенту + авторепост відгуку, лайки, коменти, delete);
**месенджер** (діалоги/тред/unread, «Написати організатору» з деталей); **медіа івенту** (постер-аплоад multipart → /uploads, галерея, відео, соцлінки);
**мобільна адмінка мейкера** (PUT/DELETE /api/events/:id: edit, hide/publish, stop-sales, cancel). **104 бекенд-тести, lines 100%.**

## 🎟️ Квитки — повний lifecycle
- **F-09a** `POST /api/tickets/:id/return` — повернення (active→returned; платний → ескроу refund; tickets_left+1). [LOW]
- **F-09b** `POST /api/tickets/:id/transfer` — подарувати (зміна owner_id за email/username отримувача). [LOW]
- **F-09c** Android: меню квитка «⋮» (Повернути / Подарувати) + підтвердження. [LOW]
- **F-09d** Перепродаж (resale): виставити на продаж / купити чужий — після live-оплати. [MEDIUM→пізніше]

## 🔎 Пошук і фільтри (Афіша)
- **F-17a** Backend: `GET /api/events?q&city&category&free&sort` (параметризовано, LIKE-екранування, тести). [DONE]
  (date_from/date_to/price_max — добавка за потреби Android-фільтрів)
- **F-17b** Android: пошук-поле + фільтр-боттомшит (місто/дата/ціна/безкоштовні/сорт). [TODO LOW]

## 👤 Онбординг + інтереси
- **F-10a** Міграція `user_prefs` (categories[], city) + `PUT/GET /api/me/prefs` (+`onboarded`-флаг). [DONE]
- **F-10b** Android: екран онбордингу після реєстрації/першого входу (FilterChip-мультивибір + місто, «Пропустити»). [DONE]

## 🧑‍🤝‍🧑 Соціальне
- **F-11a** `GET /api/users/:id` (публічний профіль, без email) + лічильники + `/events|followers|following`. [DONE]
- **F-11b** Міграція `follows` + `POST/DELETE /api/users/:id/follow` (ідемпотентно, self-follow 422). [DONE]
- **F-11c** Android: чужий профіль (Підписатись/Відписатись optimistic, події автора; лінк з деталей події). [DONE]
- **F-13a** Міграція `reviews` (rating 1..5, text, unique(event,author), upsert) + `POST /api/reviews` + `GET /api/events/:id/reviews` агрегат; рейтинг організатора в профілі. [DONE]
- **F-13b** Android: секція відгуків на деталях події (зірки+текст) + рейтинг у профілі організатора. [DONE]

## 📝 Пости/стіна (замість стрічки — рішення 2026-06-13: таб «Стрічка» прибрано)
- **F-12a'** Міграція `posts`+`post_likes`+`post_comments`; `POST /api/posts`, `GET /api/users/:id/posts` (стіна),
  автопост анонсу при створенні івенту, авторепост відгуку (replace при re-review), `DELETE /api/posts/:id`. [DONE]
- **F-12b** `POST/DELETE /api/posts/:id/like`, `GET/POST /api/posts/:id/comments`. [DONE]
- **F-12c'** Android: стіна у своєму профілі (композер, PostCard з міні-карткою івенту, лайк, коменти-екран, delete)
  + стіна в чужому профілі. [DONE]
- **F-12d** `GET /api/feed` — стрічку ПОВЕРНУТО (рішення 2026-06-13 #2): пости підписок (будь-який kind)
  ∪ пости про події з категорій інтересів (prefs); поле `reason: following|interest` (following виграє),
  свої пости виключено, blocked відфільтровано, LIMIT 50. Android: таб «Стрічка» (5 табів), PostCard з
  позначкою причини, тап автора → профіль з «Підписатись» (підписка універсальна: мейкер чи юзер). [DONE]

## 💬 Повідомлення (новий епік, 2026-06-13)
- **F-18a** Міграція `messages`; `POST /api/messages`, `GET /api/me/conversations` (last+unread), `GET /api/messages/:user_id` (тред, mark-read). [DONE]
- **F-18b** Android: ConversationsScreen (з профілю), ChatScreen (полінг 4с, бульбашки), «Написати» в чужому профілі, «Написати організатору» на деталях. [DONE]

## 🗂️ Організатор: медіа + адмінка (2026-06-13)
- **F-05Ba** `PUT /api/events/:id` (owner-only, COALESCE-patch: поля+status published|hidden+sales_closed) + `DELETE` (cancelled);
  reserve/checkout шанують sales_closed; Android EditEventScreen («Керувати» в Моїх подіях). [DONE]
- **F-05Bb** `POST /api/uploads` (multipart jpeg/png/webp ≤5MB → ./uploads, ServeDir, PUBLIC_BASE_URL);
  події: poster_url/gallery_urls/video_url/links (міграція 0012); Android photo-picker аплоад у формі створення,
  галерея/відео/лінк-чипи на деталях. [DONE]

## 🔔 Сповіщення
- **F-14a** Міграція `notifications` + fan-out на створенні події за `prefs.categories`+`city` (case-insensitive, без self-notify, best-effort). [DONE]
- **F-14b** `GET /api/me/notifications` + `POST /api/me/notifications/read`; Android: таб Сповіщення (список, mark-read, тап→деталі події). [DONE]
- **F-14c** Push (FCM) — потребує google-services + ключі. [MEDIUM→пізніше]

## 💳 Платежі live + фінанси
- **F-15a** Lemon Squeezy hosted checkout (mode=live): `checkout` → LS-URL; Android Custom Tab. [MEDIUM]
- **F-15b** LS webhook `POST /api/webhooks/lemonsqueezy` (підпис) → order paid + escrow held. [MEDIUM]
- **F-15c** Ескроу-кліринг: подія минула → held→cleared (cron/endpoint); виплати організаторам (held→paid_out, admin). [MEDIUM]
- **F-15d** Android/Web: фінанси організатора (баланси за станами ескроу, історія). [LOW]

## 🛠️ Адмінка (далі)
- **F-16a** Сторінка Events (модерація: published/hidden/blocked) + Users (block/role). [LOW]
- **F-16b** Сторінка Complaints (черга скарг) + Escrow/Payouts (uphold/refund/release). [MEDIUM]
- **F-16c** Дашборд: GMV, комісія, к-сть подій/квитків. [LOW]

## 🗂️ Організатор (далі)
- **F-05Ba** Подія: edit / stop-sales / delete (owner-only). [LOW]
- **F-05Bb** Постер-аплоад (зображення) — сховище (S3/local) + URL. [MEDIUM]

## 🧰 Інфра-добивка (доктрина §8, BLG-003)
- **F-INF-a** Адмінку — у docker-compose (контейнер). [LOW]
- **F-INF-b** Playwright e2e (admin) + Android instrumented UI. [MEDIUM]
- **F-INF-c** `scripts/ci.sh` — додати admin lint/build + android assemble стадії. [LOW]

## Орієнтовний порядок виконання
F-09(a–c) → F-17(a–b) → F-10(a–b) → F-11(a–c)+F-13 → F-14(a–b) → F-12 → F-15(live) → F-16 → добивки.
