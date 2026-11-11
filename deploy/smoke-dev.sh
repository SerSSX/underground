#!/usr/bin/env bash
# DEV smoke, run ON THE BOX by deploy-dev.sh through the public URLs (via Caddy and TLS, as a visitor).
# Usage: bash smoke-dev.sh <commit>
#
# Proves: the served API, web and admin builds are <commit> (a stale container cannot pass), the catalogue is
# served, and the core sign-in works (a demo organiser logs in and lists its events). Prints pass/fail only:
# the demo password is read from .env.dev here and never printed.
set -uo pipefail
SHA=$1
set -a; . /opt/underground-dev/.env.dev; set +a
W=$PUBLIC_WEB_URL; A=$PUBLIC_ADMIN_URL
fail=0
ok()   { echo "  ok   $1"; }
bad()  { echo "  FAIL $1"; fail=1; }
get()  { curl -fsS --max-time 30 "$@"; }

v=$(get "$W/api/version" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("build",""))' 2>/dev/null)
[ "$v" = "$SHA" ] && ok "api build is $SHA" || bad "api build is '$v', want $SHA"

get "$W/" | grep -q "/_next/static/$SHA/" && ok "web build is $SHA" || bad "web home does not serve build $SHA"
get "$A/" | grep -q "/_next/static/$SHA/" && ok "admin build is $SHA" || bad "admin home does not serve build $SHA"

n=$(get "$W/api/events" | python3 -c 'import sys,json; d=json.load(sys.stdin); print(len(d if isinstance(d,list) else d.get("items",[])))' 2>/dev/null)
[ "${n:-0}" -gt 0 ] 2>/dev/null && ok "catalogue serves $n events" || bad "catalogue is empty or broken (${n:-error})"

tok=$(python3 - "$W" <<'PY'
import json, os, sys, urllib.request
req = urllib.request.Request(sys.argv[1] + "/api/auth/login", method="POST",
    data=json.dumps({"email": "maker@underground.dev", "password": os.environ["UG_SEED_PASSWORD"]}).encode(),
    headers={"content-type": "application/json"})
try:
    print(json.load(urllib.request.urlopen(req, timeout=30))["token"])
except Exception:
    pass
PY
)
if [ -n "$tok" ]; then
  m=$(get -H "authorization: Bearer $tok" "$W/api/me/events" | python3 -c 'import sys,json; d=json.load(sys.stdin); print(len(d if isinstance(d,list) else d.get("items",[])))' 2>/dev/null)
  [ "${m:-0}" -gt 0 ] 2>/dev/null && ok "organiser signs in and sees $m own events" || bad "organiser's events missing (${m:-error})"
else
  bad "demo organiser cannot sign in"
fi

h=$(curl -sS -o /dev/null -D - --max-time 30 "$W/" | tr -d '\r' | grep -i '^x-robots-tag:' || true)
echo "$h" | grep -qi noindex && ok "noindex header present" || bad "noindex header missing"

[ $fail = 0 ] && echo "SMOKE GREEN $SHA" || echo "SMOKE RED $SHA"
exit $fail
