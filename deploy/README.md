# Underground — DEV: розгортання і експлуатація

DEV-стенд живе на спільному боксі `65.108.87.247` (Hetzner CAX11, arm64, 4 ГБ) за спільним Caddy, який також
обслуговує живі продукти (msgview та інші). Усе, що належить Underground, лежить у `/opt/underground-dev/`.
Прод — лише за командою власника (rules §9); його тут ще немає.

| Що | Де |
|---|---|
| Веб + API (одне джерело) | https://underground-dev.65-108-87-247.sslip.io |
| Адмінка | https://underground-admin-dev.65-108-87-247.sslip.io |
| Конфіг із секретами | `/opt/underground-dev/.env.dev` (600; створюється на боксі, ніколи не друкується) |
| Поточний реліз | `/opt/underground-dev/release.env` (`UG_TAG=<коміт>`) |
| Журнал релізів | `/opt/underground-dev/releases.jsonl` (час, коміт, попередній, смоук, команда відкату) |
| Дані | `/opt/underground-dev/data/{pg,uploads}` |
| Caddy | блоки в `/opt/caddy/Caddyfile` (див. `caddy-underground-dev.snippet`) |

## Як код потрапляє на DEV

1. Пуш у гілку `claude/underground-mvp` → CI (`.github/workflows/ci.yml`): тести API на Postgres, typecheck і
   збірка веба й адмінки, аудит залежностей, Android (коли змінено `android/`). Якщо все зелене, CI збирає три
   arm64-образи на arm64-раннері й кладе їх артефактом `images` (живе 3 дні).
2. `bash deploy/deploy-dev.sh` на маку: бере найновіший коміт гілки, **відмовляє**, якщо CI цього коміту не
   зелений, качає образи саме цього коміту, звіряє контрольні суми, вантажить на бокс, перемикає стенд і
   проганяє смоук (`smoke-dev.sh`). Червоний смоук сам повертає попередній реліз. Кожен запуск пише рядок у
   `releases.jsonl`.
3. Відкат: `bash deploy/deploy-dev.sh <попередній коміт>` (команда записана в журналі).

На боксі нічого не компілюється: там живі продукти, а збірка Rust з'їла б пам'ять і процесор.

## Перший запуск на чистому боксі

```bash
bash deploy/bootstrap-dev.sh
```

```bash
bash deploy/deploy-dev.sh
```

```bash
bash deploy/seed-dev.sh
```

```bash
bash deploy/deploy-dev.sh
```

Перший `deploy-dev.sh` іде на порожню базу, тож його смоук червоний (немає подій і демо-організатора); стенд
при цьому лишається піднятим. Після `seed-dev.sh` другий прогін того самого коміту — зелений.

## Вхід на DEV

Пароль демо-акаунтів згенеровано на боксі. Власник бачить його у своєму терміналі:

```bash
bash deploy/show-dev-login.sh
```

Акаунти: `user@underground.dev` (гість), `maker@underground.dev` (організатор усіх засіяних подій),
`admin@underground.dev` (адмін). Агент цей скрипт не запускає.

## Операції

```bash
ssh -i ~/.ssh/hetzner root@65.108.87.247 'docker ps --filter name=underground-dev; docker logs --tail 100 underground-dev-api'
```

```bash
ssh -i ~/.ssh/hetzner root@65.108.87.247 'tail -5 /opt/underground-dev/releases.jsonl'
```

```bash
ssh -i ~/.ssh/hetzner root@65.108.87.247 'docker stats --no-stream --format "{{.Name}} {{.MemUsage}}" | grep underground'
```

## Чого цей стенд НЕ вміє (на 12 лист. 2026)

- **Пошта не надсилається** — код входу листом не приходить; вхід паролем (акаунти вище). Крок 1 плану.
- **Google-вхід не налаштований.** Крок 5.
- **Образи веба й адмінки зібрані під DEV-адреси** (`NEXT_PUBLIC_*` вшиваються під час збірки). Перед продом
  це треба перевести на конфіг під час запуску (rules §6 «один артефакт для всіх середовищ»). Крок 6.
- **Бекапів бази DEV немає** (дані демо, одноразові); перед сідом робиться `pg_dump` у `/opt/backups/`.
- Ліміти пам'яті: db 256 МБ, api 256 МБ, web 256 МБ, admin 192 МБ.
