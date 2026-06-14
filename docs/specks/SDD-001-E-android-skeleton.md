# SDD-001-E — Android skeleton

**Backlog:** TASK-001-E  **Parent:** TASK-001
**Impacted UCs:** UC-002 (Афіша read)

## Scope / Non-goals
**Scope:** dark-only Material3; нижня навігація 5 табів; Афіша з `GET /api/events` (Retrofit+kotlinx.serialization, Coil);
стани loading/empty/error; base URL через `BuildConfig.API_BASE_URL`. JVM unit-тести логіки.
**Non-goals:** Hilt; реальні екрани інших табів; auth; instrumented UI + device-matrix (follow-up).

## Design
- `data/Event.kt` — `@Serializable EventDto` з `@SerialName` для snake_case.
- `data/EventApi.kt` — `GET api/events`. `data/Network.kt` — Retrofit factory. `data/EventRepository.kt`.
- `ui/theme/*` — фіксований `darkColorScheme`. `ui/AppScaffold.kt` — `NavHost` + `NavigationBar`.
- `ui/afisha/AfishaViewModel.kt` — `StateFlow<AfishaUiState>` (Loading/Success/Empty/Error). `AfishaScreen.kt`, `components/EventCard.kt`.
- `util/Format.kt` — дата (зріз ISO до `YYYY-MM-DD`, бо minSdk24 без java.time), ціна.
- `res/xml/network_security_config.xml` — cleartext для localhost/127.0.0.1/10.0.2.2 (adb reverse).

## Acceptance Criteria
- **AC-1** `assembleDebug` зелений; APK ставиться й стартує (темна тема, нижня навігація).
- **AC-2** Афіша на старті тягне `/api/events`; показує картки (через `adb reverse tcp:8080`).
- **AC-3** Порожній список → empty-стан; помилка мережі → error-стан із «Повторити».
- **AC-4** Base URL — з `BuildConfig`, не хардкод у коді.

## Security Acceptance Criteria
- **SEC-1** Cleartext дозволено лише для localhost/loopback (network-security-config), не глобально.
- **SEC-2** Десеріалізація терпима до зайвих полів (`ignoreUnknownKeys`), не падає на несподіваному JSON.

## Test Matrix
| AC | JVM unit | Build/Install | Manual on device |
|---|---|---|---|
| AC-1 | — | ✅ assembleDebug + install | ✅ |
| AC-2 | ✅ DTO parse (snake_case) | — | ✅ |
| AC-3 | ✅ стан-редюсер (empty/error) | — | ✅ |
| AC-4 | — | ✅ BuildConfig field | — |
| SEC-2 | ✅ ignoreUnknownKeys | — | — |

> Instrumented Compose UI + повна device-matrix (§7.3) — окремий follow-up TASK (гап, §9.1).
