#!/usr/bin/env bash
# Build, install, and run the Android app on a USB-connected device, tunnelling the
# dev API to the phone via `adb reverse` (rules §10: base URL configurable, not hard-coded).
set -euo pipefail
cd "$(dirname "$0")/.."

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
ADB="$ANDROID_HOME/platform-tools/adb"
APK="android/app/build/outputs/apk/debug/app-debug.apk"
PKG="com.underground.app"

[ -f .env ] || cp .env.example .env
# shellcheck disable=SC1091
source .env

echo "== ensure API stack is up =="
docker compose up -d >/dev/null
for _ in $(seq 1 30); do curl -fsS "localhost:${API_PORT}/health" >/dev/null 2>&1 && break; sleep 2; done

echo "== device =="
"$ADB" wait-for-device
"$ADB" devices -l | sed -n '2p'

echo "== USB tunnel: phone localhost:${API_PORT} -> host API =="
"$ADB" reverse --remove-all || true
"$ADB" reverse "tcp:${API_PORT}" "tcp:${API_PORT}"

echo "== build =="
(cd android && ./gradlew :app:assembleDebug --console=plain -q)

echo "== install + launch =="
"$ADB" install -r "$APK"
"$ADB" shell am start -n "$PKG/.MainActivity" >/dev/null

echo "app running on device — API tunnelled on tcp:${API_PORT}"
