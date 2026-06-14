# underground

Mobile app (Android) with a Rust backend.

## Layout

```
underground/
├── api/        # Rust backend — Axum + Tokio
└── android/    # Android client — Kotlin + Jetpack Compose
```

## Backend (`api/`)

Requires the Rust toolchain (`rustup`).

```bash
cd api
cargo run          # serves on http://0.0.0.0:8080
```

Endpoints:
- `GET /health` → `{"status":"ok"}`
- `GET /api/version` → name + version

Configurable via `PORT` (default `8080`) and `RUST_LOG` (default `info`).

## Android (`android/`)

Requires JDK 17 and the Android SDK. Set the SDK path in `android/local.properties`:

```
sdk.dir=/Users/<you>/Library/Android/sdk
```

```bash
cd android
./gradlew assembleDebug
```
