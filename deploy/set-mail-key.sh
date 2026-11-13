#!/usr/bin/env bash
# Switch on account mail (password-reset links, sign-in codes) on the Underground DEV stand. TASK-304.
#
#   bash deploy/set-mail-key.sh
#
# Run by the OWNER in his own terminal. Agents never run it: the Resend key is a secret (rules §9).
# - asks for the Resend API key without echoing it, and for the sender address;
# - sends both to the box over ssh on STDIN (never on a command line, never printed), where they replace the
#   RESEND_API_KEY / EMAIL_FROM lines of /opt/underground-dev/.env.dev (mode 600);
# - recreates the api container so it reads them, then checks GET /api/features answers "email": true.
# To switch mail off again: run it and enter an empty key (both lines are then written empty).
# Needs a deployed release that includes TASK-304 (its docker-compose.dev.yml passes the two keys to the api).
set -euo pipefail
HOST=${UG_DEV_HOST:-root@65.108.87.247}
KEY=${UG_DEV_SSH_KEY:-$HOME/.ssh/hetzner}
URL=${UG_DEV_URL:-https://underground-dev.65-108-87-247.sslip.io}
DEFAULT_FROM='Underground <no-reply@ssvstudio.dev>'

read -rs -p "Ключ Resend (re_…, на екрані не видно; порожньо = вимкнути пошту): " RK
echo
if [ -n "$RK" ]; then
  [[ $RK =~ ^re_[A-Za-z0-9_]{8,200}$ ]] || { echo "Ключ має виглядати як re_… (лише латинські літери, цифри, _). Нічого не змінено."; exit 1; }
  read -r -p "Відправник [$DEFAULT_FROM]: " FROM
  FROM=${FROM:-$DEFAULT_FROM}
  # .env.dev is read by docker compose: no quote, $ or backslash in the value, one line, has an @.
  [[ $FROM == *@* && $FROM != *"'"* && $FROM != *'"'* && $FROM != *'$'* && $FROM != *'\'* ]] \
    || { echo "Адреса відправника виглядає неправильно. Нічого не змінено."; exit 1; }
else
  FROM=""
fi

# The remote script is code only; the two values arrive on its stdin.
REMOTE='set -euo pipefail
cd /opt/underground-dev
IFS= read -r RK || true
IFS= read -r FROM || true
umask 077
TMP=$(mktemp .env.dev.XXXXXX)
# grep -v: exit 1 only means every line was one of the two keys (nothing left); 2 is a real error.
RC=0
grep -v -E "^(RESEND_API_KEY|EMAIL_FROM)=" .env.dev > "$TMP" || RC=$?
if [ "$RC" -gt 1 ]; then rm -f "$TMP"; echo "ERROR: .env.dev could not be read; nothing changed" >&2; exit 1; fi
if [ -n "$RK" ]; then
  printf "RESEND_API_KEY=%s\nEMAIL_FROM='"'"'%s'"'"'\n" "$RK" "$FROM" >> "$TMP"
else
  printf "RESEND_API_KEY=\nEMAIL_FROM=\n" >> "$TMP"
fi
chmod 600 "$TMP"
mv "$TMP" .env.dev
docker compose --env-file .env.dev --env-file release.env -f docker-compose.dev.yml up -d --no-deps --force-recreate --wait --wait-timeout 120 api >/dev/null
# Presence only, never the value.
LEN=$(docker exec underground-dev-api sh -c "printf %s \"\$RESEND_API_KEY\" | wc -c" | tr -d " ")
echo "api перезапущено; довжина ключа в контейнері: $LEN"'
printf '%s\n%s\n' "$RK" "$FROM" | ssh -i "$KEY" -o BatchMode=yes -o ConnectTimeout=15 "$HOST" "bash -c $(printf %q "$REMOTE")"
unset RK

FEATURES=$(curl -fsS --max-time 20 "$URL/api/features")
echo "GET /api/features → $FEATURES"
if [ -n "$FROM" ]; then
  [[ $FEATURES == *'"email":true'* ]] && echo "Пошта увімкнена. Перевірте: $URL/forgot-password" \
    || { echo "Пошта НЕ увімкнулась: API досі каже email=false (чи розгорнуто реліз із TASK-304?)"; exit 1; }
else
  [[ $FEATURES == *'"email":false'* ]] && echo "Пошту вимкнено." || { echo "API досі каже email=true"; exit 1; }
fi
