# TASK-001-E — Android skeleton (dark theme + nav + API client)

**Status:** DONE
**Created:** 2026-06-12  **Updated:** 2026-06-12
**Parent:** TASK-001  **Speck:** docs/specks/SDD-001-E-android-skeleton.md

## Verification (2026-06-12)
`./gradlew testDebugUnitTest assembleDebug` → BUILD SUCCESSFUL; 7 JVM-тестів зелені (FormatTest×4, EventDtoTest×3 — DTO snake_case+ignoreUnknownKeys, reduce empty/success).
Встановлено на фізичний **Motorola moto g05** (`adb install`), `adb reverse tcp:8080`; апка стартує: dark theme + нижня навігація (5 табів),
Афіша показує живі події з `/api/events` (STAND UP 750 UAH·32 left; LOUNA 650 UAH·173 left). Без крашів (logcat чистий).
SEC: cleartext лише для loopback (network-security-config); `ignoreUnknownKeys`.
Відоме: постери з зовнішнього picsum не довантажились (залежить від інтернету телефона) — додати placeholder/error для AsyncImage (follow-up).
Гап: Compose instrumented UI + device-matrix (§7.3) — окремий follow-up TASK.

## Enhancement (2026-06-12, на запит користувача «пропрацюй дизайн + наповни контентом»)
За прямою вказівкою користувача (§0: User > порядок rules) розширено до раннього зрізу UC-002/UC-003:
- **Дизайн під концепцію:** UNDERGROUND-хедер, фільтр-чіпи категорій, картки з градієнт-постером за кольором категорії
  (надійний fallback, коли зовнішнє зображення не вантажиться), пілюлі категорії/«N left», дата, ціна-акцент, зірка.
- **Екран деталей події** (UC-003 read) + навігація Афіша→Деталі: постер, локація, **опис**, ціна, «Купити квиток» (stub-toast).
- **Контент:** бекенд `description` (міграція 0002) + сід 11 подій (`api/seeds/dev_events.sql`, `scripts/seed-dev.sh`).
- **USB-тунель:** `scripts/device-run.sh` (compose up → `adb reverse` → build → install → launch).
- Тести: +3 (filterByCategory×2) — разом 9 JVM-тестів зелені. Перевірено на moto g05 (скріни Афіша + Деталі).
> Формалізація у TASK-003 (UC-002) / TASK-004 (UC-003) специфікаціях — наступним кроком феатур-фази.

## Raw Request
> Листя INFRA з TASK-001 + прохання користувача побачити апку на телефоні: каркас Android-клієнта.

## Refined Description
**Scope:** dark-only Material3 тема; нижня навігація (5 табів: Афіша/Стрічка/Квитки/Сповіщення/Профіль);
екран **Афіша** тягне `GET /api/events` (Retrofit + kotlinx.serialization) і показує картки подій (Coil-постер,
назва, категорія·місто, дата, ціна, лишилось). Loading/empty/error стани. Base URL — через `BuildConfig` (не хардкод, §10).
JVM unit-тести (DTO-парсинг snake_case, формат дати/ціни).
**Non-goals:** Hilt DI (відкладено; манульна інстанціація); реальні екрани решти табів (заглушки); auth (TASK-002);
Compose instrumented UI-тести + device-matrix (окремий follow-up TASK — гап задокументовано).
**Impacted UCs:** UC-002 (Афіша, read). **Dependencies:** TASK-001-D (API).

## Estimation
**Level:** LOW  **Justification:** каркас + один екран з мережею; передбачуваний обсяг.

## Speck Reference
docs/specks/SDD-001-E-android-skeleton.md
