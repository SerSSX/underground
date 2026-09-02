# Underground — DEV deployment runbook

The dev stand runs on the shared Hetzner box (`159.69.159.230`) behind the Caddy that already
fronts six other products. Everything Underground owns lives under `/opt/underground-dev/`.

| Що | Де |
|---|---|
| Публічний веб + API | https://underground.159-69-159-230.sslip.io |
| Адмінка (той самий API) | https://underground-admin.159-69-159-230.sslip.io |
| Джерела на боксі | `/opt/underground-dev/src` (rsync з робочої копії) |
| Конфіг зі секретами | `/opt/underground-dev/.env.dev` (600, **поза** `src/`, rsync його не чіпає) |
| Дані | `/opt/underground-dev/data/{pg,uploads}` |
| Caddy | блок у спільному `/opt/signalops/caddy/Caddyfile` (див. `caddy-underground-dev.snippet`) |

Тестові акаунти (пароль `underground`): `user@underground.dev`, `maker@underground.dev`
(організатор усіх 20 засіяних подій), `admin@underground.dev` (роль `ADMIN`).

## Розгорнути / оновити

```bash
bash deploy/publish-dev.sh
```

Скрипт: rsync джерел → `docker compose build` → `up -d` → димовий тест. Він **не** створює
`.env.dev` — секрети мінтяться на боксі один раз (нижче) і ніколи не комітяться (§11).

## Перший запуск на чистому боксі

```bash
ssh -i ~/.ssh/hetzner root@159.69.159.230 '
  mkdir -p /opt/underground-dev/{src,data/pg,data/uploads}
  chown -R 10001:10001 /opt/underground-dev/data/uploads   # uid non-root юзера в образі API
'
# 1) джерела
bash deploy/publish-dev.sh --no-build
# 2) секрети — генеруються НА боксі, hex-only (compose з'їдає `$` у .env-файлі)
ssh -i ~/.ssh/hetzner root@159.69.159.230 '
  cd /opt/underground-dev
  sed -e "s|^POSTGRES_PASSWORD=.*|POSTGRES_PASSWORD=$(openssl rand -hex 24)|" \
      -e "s|^JWT_SECRET=.*|JWT_SECRET=$(openssl rand -hex 32)|" \
      src/deploy/.env.dev.example > .env.dev && chmod 600 .env.dev
'
# 3) збірка + підйом
bash deploy/publish-dev.sh
# 4) Caddy — бекап, дописати блок, ЗВАЛІДУВАТИ, reload (не restart: файл фронтить 6 інших продуктів)
ssh -i ~/.ssh/hetzner root@159.69.159.230 '
  CF=/opt/signalops/caddy/Caddyfile
  cp -a $CF /opt/backups/Caddyfile.pre-underground-$(date +%F-%H%M%S)
  cat /opt/underground-dev/src/deploy/caddy-underground-dev.snippet >> $CF
  docker exec signalops-caddy-caddy-1 caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile &&
  docker exec signalops-caddy-caddy-1 caddy reload   --config /etc/caddy/Caddyfile --adapter caddyfile
'
```

## Засіяти демо-дані

```bash
ssh -i ~/.ssh/hetzner root@159.69.159.230 '
  set -a; . /opt/underground-dev/.env.dev; set +a
  docker exec -i underground-dev-db psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -q \
    < /opt/underground-dev/src/api/seeds/dev_events.sql
'
API=https://underground.159-69-159-230.sslip.io
for e in user maker admin; do
  curl -s -o /dev/null -w "$e %{http_code}\n" -X POST "$API/api/auth/register" \
    -H 'content-type: application/json' \
    -d "{\"email\":\"$e@underground.dev\",\"password\":\"underground\",\"name\":\"$e\"}"
done
ssh -i ~/.ssh/hetzner root@159.69.159.230 '
  set -a; . /opt/underground-dev/.env.dev; set +a
  docker exec underground-dev-db psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -q \
   -c "UPDATE events SET organizer_id=(SELECT id FROM users WHERE email='"'"'maker@underground.dev'"'"') WHERE organizer_id IS NULL;" \
   -c "UPDATE users SET role='"'"'ADMIN'"'"' WHERE email='"'"'admin@underground.dev'"'"';"
'
```

## Операції

```bash
# логи / стан
ssh -i ~/.ssh/hetzner root@159.69.159.230 'docker ps --filter name=underground-dev; docker logs --tail 100 underground-dev-api'
# перезапуск однієї служби
ssh ... 'cd /opt/underground-dev/src && docker compose --env-file /opt/underground-dev/.env.dev -f deploy/docker-compose.dev.yml restart api'
# знести стенд разом з даними (dev — дані одноразові)
ssh ... 'cd /opt/underground-dev/src && docker compose --env-file /opt/underground-dev/.env.dev -f deploy/docker-compose.dev.yml down && rm -rf /opt/underground-dev/data'
```

**Відкат** — образи тегуються тільки `latest`, тож відкат = викотити попередній коміт тим самим
`publish-dev.sh`. Для дев-стенду цього досить; для прод-стенду потрібні теговані образи (не зроблено).

## Що цей стенд НЕ вміє

- **Реальних грошей немає** — платіжний провайдер у режимі Noop/Test, LemonSqueezy не під'єднано.
- **Пошта і push не відправляються** — SendGrid/FCM не налаштовані; воркери лише пишуть у лог.
  Отже: коди входу листом не приходять — для входу користуйся паролем (акаунти вище).
- **Google OAuth не налаштований** — вхід лише email+пароль.
- Бекапів БД немає, ресурсних лімітів на контейнерах немає (бокс спільний — це ризик для сусідів).
