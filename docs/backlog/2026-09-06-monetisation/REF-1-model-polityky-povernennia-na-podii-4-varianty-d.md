# REF-1 — Модель політики повернення на події: 4 варіанти, дефолти, правило «лише пом'якшити»

| Поле | Значення |
|---|---|
| **Фаза** | Фаза 2 — повернення |
| **Estimation** | M |
| **Status** | `INTAKE` |
| **Критичний шлях до першого платежу** | так |
| **Depends on** | — |

## Навіщо

Політики повернення в коді НЕМАЄ взагалі: у таблиці events (міграція 0001 + усі 39) немає жодної колонки про рефанд, а tickets.rs::return_ticket гейтить повернення виключно по escrow_status ('cleared'|'paid_out' → 409 «термін повернення квитка минув»). Тобто фактична політика сьогодні — «повертай, поки sweep не докрутив кошти», і вона залежить від clearing_days, а не від рішення організатора. Вимога власника №3 (організатор обирає політику, покупець бачить умови ДО оплати) без цієї моделі не має де жити. Це фундамент усієї фази: REF-2…REF-9 читають саме ці поля.

## Як це працює

Організатор при створенні події обирає, що буде з грошима, якщо покупець передумає. Варіантів рівно чотири, більше не буде:

1. `full` — «Повне повернення будь-коли до початку події».
2. `until` — «Повне повернення до N годин перед початком» (дефолт для нової платної події: N = 24).
3. `partial` — «Повернення P% до N годин перед початком» (P = 1…99).
4. `none` — «Без повернення».

Дефолт для НОВОЇ платної події: `until`, 24 години, 100%, сервісний збір не повертається. Організатор, який нічого не чіпав, отримує людяну політику, а не «без повернення».

БЕЗКОШТОВНА подія (cost_type='free') політику не має взагалі: реєстрацію можна скасувати будь-коли до початку. Поля в БД зберігаються, але API їх ігнорує і віддає покупцеві текст «Безкоштовна реєстрація — скасуйте будь-коли до початку події».

Дві залізні гарантії поверх вибору організатора:
- Скасування або перенесення події → 100% повернення завжди, незалежно від політики. Цю гілку організатор вимкнути не може (REF-8 її виконує).
- Після того як з'явилось перше ОПЛАЧЕНЕ замовлення, політику можна змінити ЛИШЕ на м'якшу. Спроба зробити жорсткішою — 409 з текстом «Політику повернення вже видно покупцям цієї події. Її можна зробити тільки м'якшою: збільшити відсоток або скоротити строк». М'якшість рахується формально: нова політика м'якша або рівна, якщо new_percent >= old_percent І new_cutoff_hours <= old_cutoff_hours (де `none` = 0% / ∞ годин, `full` = 100% / 0 годин).

API віддає не тільки сирі поля, а й ГОТОВИЙ рядок українською та англійською (`refund_policy_text`) і обчислений `refundable_until` для цієї події — щоб три клієнти не писали три різні формулювання (це і є вимога «покупець розуміє, повернуть йому кошти чи ні»).

## Зміни в API

Новий модуль `api/src/refunds.rs` — чиста логіка без БД, щоб її можна було покрити тестами під гейт 100% line coverage:
- `pub struct RefundPolicy { kind: String, cutoff_hours: i32, percent: i32, fees_returned: bool }`
- `pub fn validate(p: &RefundPolicy) -> Result<(), AppError>` — kind ∈ full|until|partial|none; percent 0..=100 (для `partial` строго 1..=99); cutoff_hours 0..=8760; для `full`/`none` cutoff і percent нормалізуються (full → 0/100, none → 0/0).
- `pub fn is_softening(old: &RefundPolicy, new: &RefundPolicy) -> bool` — правило вище.
- `pub fn refundable_until(p: &RefundPolicy, starts_at: Option<DateTime<Utc>>) -> Option<DateTime<Utc>>` — `starts_at - cutoff_hours`; для `none` → None; для `full` → starts_at.
- `pub fn describe(p: &RefundPolicy, ctx: DescribeCtx, lang: &str) -> String` — дослівні тексти живуть тут (використовує REF-4).

`api/src/events.rs`:
- `CreateEventReq` += `refund_policy: Option<String>`, `refund_cutoff_hours: Option<i32>`, `refund_percent: Option<i32>`, `refund_fees_returned: Option<bool>`; відсутні → дефолти (until/24/100/false).
- `UpdateEventReq` += ті самі поля; у `update_event` перед UPDATE — виклик `refunds::is_softening`, якщо `EXISTS (SELECT 1 FROM orders WHERE event_id = $1 AND status = 'paid')`.
- `Event` (серіалізована відповідь) += ті самі 4 поля + `refund_policy_text: String` + `refundable_until: Option<DateTime<Utc>>`.
- `require_owner` вже є — авторизацію не переписуємо.

## Зміни в базі

Нова міграція `0040_refund_policy.sql`:
ALTER TABLE events
  ADD COLUMN refund_policy TEXT NOT NULL DEFAULT 'until' CHECK (refund_policy IN ('full','until','partial','none')),
  ADD COLUMN refund_cutoff_hours INT NOT NULL DEFAULT 24 CHECK (refund_cutoff_hours BETWEEN 0 AND 8760),
  ADD COLUMN refund_percent INT NOT NULL DEFAULT 100 CHECK (refund_percent BETWEEN 0 AND 100),
  ADD COLUMN refund_fees_returned BOOLEAN NOT NULL DEFAULT FALSE;

Бекфіл наявних подій: явним UPDATE у ТІЙ САМІЙ міграції ставимо існуючим платним подіям refund_policy='until', cutoff=24, percent=100 (щоб не покладатися на DEFAULT для рядків, створених до міграції, і не отримати різну поведінку для старих і нових подій).

## Поверхні

**Веб.** Немає (форма — REF-3, показ — REF-4). У цій задачі web лише перестає падати на нових полях у типі Event: `web/lib/api.ts` розширюється, `web/app/refunds/page.tsx` поки не чіпаємо.

**Android.** Немає (форма — REF-3). Розширюється лише `android/.../data/Event.kt` новими nullable-полями, щоб парсер не ламався на новому JSON.

**Telegram Mini App.** Немає — Telegram Mini App не існує; політика приїде туди разом із TMA у Фазі 5, читаючи ті самі поля API.

## Критерії приймання (AC)

- POST /api/events без жодного refund_* поля створює платну подію з refund_policy='until', refund_cutoff_hours=24, refund_percent=100, refund_fees_returned=false.
- GET /api/events/:id для такої події повертає refundable_until = starts_at мінус 24 години і refund_policy_text = «Повне повернення до 14 бер, 20:00 (за 24 год до початку). Після цього часу повернення неможливе.»
- PATCH з refund_policy='partial', refund_percent=0 повертає 400 (для partial дозволено лише 1..99); percent=100 з kind='partial' також 400 — це `until`.
- PATCH з refund_cutoff_hours=-1 або 9000 повертає 400.
- Подія має оплачене замовлення; PATCH refund_percent зі 100 на 50 повертає 409 з текстом «Політику повернення вже видно покупцям цієї події…», і в БД поля не змінились.
- Та сама подія: PATCH refund_cutoff_hours з 24 на 2 (пом'якшення) повертає 200 і зберігається.
- Подія БЕЗ оплачених замовлень: PATCH refund_policy='none' повертає 200 — до першого продажу дозволено будь-яку зміну.
- Для cost_type='free' refund_policy_text = «Безкоштовна реєстрація — скасуйте будь-коли до початку події», незалежно від значень колонок.
- refunds::refundable_until для kind='none' повертає None; для kind='full' повертає рівно starts_at.
- Міграція 0040 на базі з наявними платними подіями лишає їх з until/24/100 (перевірка SELECT після міграції), а не з випадковими значеннями.

## Що має бути відхилено (SEC)

- PATCH політики користувачем, який не є організатором події → 403; поля в БД без змін (шлях через наявний require_owner, не через новий код).
- Спроба зробити політику ЖОРСТКІШОЮ після першого оплаченого замовлення → 409, а не тихе застосування. Перевірити всі три напрямки: percent вниз, cutoff вгору, kind → 'none'.
- Спроба обійти правило через послідовність двох PATCH (пом'якшити, потім вернути назад) → другий PATCH 409, бо порівняння завжди з ПОТОЧНИМ станом у БД.
- refund_percent = 101, -5, або нечислове → 400, не паніка серіалізатора.
- refund_cutoff_hours = 2^31-1 → 400 (верхня межа 8760 годин = рік), щоб арифметика starts_at - interval не переповнилась.
- Клієнт передає refundable_until або refund_policy_text у тілі PATCH → поля ігноруються, значення обчислює сервер.
- Довільний рядок у refund_policy ('anything') → 400 на рівні валідації, а не 500 від CHECK-констрейнта БД.
