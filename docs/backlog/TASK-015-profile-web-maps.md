# TASK-015 — Профіль/налаштування, веб-кабінет CRUD, карти, QR-скан, фікс адмінки

> Запит користувача (2026-06-14). Делівері частинами з гейтами (тести 100%, clippy/fmt, деплой).

## Проблеми/запити
1. **Адмінка → Платіжки** відкриває сторінку БЕЗ Shell-навігації (тупик, не повернутись). Фікс: обгорнути в `Shell`.
2. **Веб-частина для юзерів/організаторів неповна** — `/organizer` лише читання. Треба ПОВНИЙ CRUD подій з вебу
   (створити/редагувати/сховати/стоп-продажі/скасувати, постер-аплоад) — усе, що в мобайлі, окрім QR-скану.
3. **Профіль: немає налаштувань.** Не можна: змінити імʼя/біо/аватар, перемкнути статус Користувач⇄Організатор.
   Кнопка «Вийти» прямо на профілі → випадковий вихід. Сховати у Налаштування (шестерня/бургер).
4. **Дуальність юзер⇄мейкер** = чистий лейбл (функціонально однакові). Самостійний тумблер «Режим організатора».
5. **Адреса події → Google Maps:** адреса в картці/деталях клікабельна, відкриває Google Maps.
6. **QR-сканер** перемикає орієнтацію в landscape + незручно вийти. Лочимо portrait + чистий back.

## План
### Backend
- Міграція 0013: `users.is_organizer BOOLEAN NOT NULL DEFAULT false`.
- `User`/`PublicProfile` + усі SELECT — додати `is_organizer`.
- `PATCH /api/me` — оновити `name?`, `bio?`, `avatar_url?`, `is_organizer?` (COALESCE, валідація). Тести.

### Mobile
- `UserDto`: +`isOrganizer`. `AuthApi.patchMe`. `AuthViewModel.updateUser` (оновлює LoggedIn state).
- **SettingsScreen** (нова): аватар-аплоад, імʼя, біо, тумблер «Режим організатора», «Вийти» (danger, внизу).
- **ProfileScreen**: топ-бар із шестернею → Settings; «Вийти» прибрати звідси; бейдж Користувач/Організатор за `is_organizer`.
- **Maps** util: `mapsUrl(address, city)` → `https://www.google.com/maps/search/?api=1&query=…`. Клік по адресі на деталях.
- **QR**: `PortraitCaptureActivity` + manifest `screenOrientation=portrait`; `setOrientationLocked(true)`.

### Web (admin/ Next.js)
- Payments → `Shell`.
- Shell: fetch `/api/me`, **роль-залежна навігація** (адмін-лінки лише ADMIN; «Кабінет організатора» всім).
- `/organizer`: список + «Створити» + «Редагувати»/«Відвідувачі» на кожній події.
- `/organizer/new`, `/organizer/[id]/edit`: спільна `EventForm` (всі поля + постер-аплоад + статус/стоп-продажі/скасувати).

## Не в цій сесії
- Places Autocomplete (потребує Google API key) — зараз клік-у-карти за текстовим запитом (без ключа).
- Figma-звірка (TASK-014-F, свіжа сесія з MCP).

## Статус
- 2026-06-14: ВИКОНАНО. Backend: міграція 0013 `is_organizer`, `PATCH /api/me`, `is_organizer` у User+PublicProfile,
  +6 тестів (121 загалом, lines+functions 100%). Mobile: SettingsScreen (аватар-аплоад/імʼя/біо/тумблер
  організатора/вихід), ProfileScreen (шестерня→Settings, аватар, біо, бейдж Користувач/Організатор, без
  кнопки виходу), Maps util + клік адреси→Google Maps на деталях, PortraitCaptureActivity (QR portrait+back),
  людські дати з часом. Web: payments→Shell (фікс тупика), роль-залежна навігація через `/api/me`,
  organizer CRUD (`/organizer/new`, `/organizer/[id]/edit`, EventForm з постер-аплоадом, hide/stop-sales/cancel),
  home→роль-редірект. Верифіковано на moto g05 (профіль/налаштування/тумблер→бейдж/карти/гейт відгуків/unread-бейдж).
- Follow-up: Places Autocomplete (потребує Google API key); Figma-звірка TASK-014-F (свіжа сесія з MCP).
