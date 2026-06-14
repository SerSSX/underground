# TASK-007 — QR-сканер на вході (валідація квитка)

**Status:** DONE
**Created:** 2026-06-12  **Updated:** 2026-06-12
**Parent:** TASK-001 (UC-007)

## Scope
Організатор сканує QR квитка на вході → валідація (active→used, single-use). Бекенд + Android-сканер.

## Verification (2026-06-12)
Backend: `POST /api/tickets/validate` (organizer-only, транзакція `FOR UPDATE`, active→`used`+`validated_at`); 403 не-організатору, 404 невідомий, 401 без токена.
**38 тестів** зелені (organizer→valid→already_used; non-organizer 403; unknown 404 / no-token 401). clippy/fmt чисто.
Curl-смоук: user бере Poetry-квиток → maker(організатор) validate → `valid:true,ok` → повторно `already_used` → user(не-організатор) → 403.
Android: ZXing `zxing-android-embedded` сканер, CAMERA-дозвіл (runtime), кнопка QR у хедері «Мої події» → камера → `validate` → діалог результату (✓ Дійсний / ✗ Уже використаний / ✗ Не ваша подія / ✗ Невідомий). Build чистий.
**Перевірено на moto g05:** «Мої події» показує іконку сканера; тап → відкривається `CaptureActivity` (камера зі скан-лінією). Фізичний скан QR не автоматизується (нема контролю телефона в просторі) — повний ланцюг доведено бекенд-тестами+curl.
SEC: валідація лише організатором події (403); single-use через транзакцію.
Follow-up: екран сканера в-апп замість окремої Activity (опц.); лічильник сканувань на події.
