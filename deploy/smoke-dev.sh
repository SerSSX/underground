#!/usr/bin/env bash
# DEV smoke, run ON THE BOX by deploy-dev.sh through the public URLs (Caddy + TLS, as a visitor).
# Usage: bash smoke-dev.sh <commit>
#
# Proves: the served API, web and admin builds are <commit> (a stale container cannot pass); each page's own
# JS chunk loads (a page whose scripts 404 is dead even when it answers 200); the catalogue is served; a demo
# organiser signs in and lists its events; DEV is marked noindex. Prints pass/fail only: passwords and tokens
# stay inside this process and never reach a command line or the output.
set -uo pipefail
. /opt/underground-dev/.env.dev
SHA="$1" W="$PUBLIC_WEB_URL" A="$PUBLIC_ADMIN_URL" PW="$UG_SEED_PASSWORD" exec python3 - <<'PY'
import json, os, re, sys, urllib.request, urllib.error

sha, W, A = os.environ["SHA"], os.environ["W"], os.environ["A"]
fails = []

def fetch(url, data=None, headers=None):
    req = urllib.request.Request(url, data=data, headers=headers or {}, method="POST" if data else "GET")
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.status, dict(r.headers), r.read()

def check(name, fn):
    try:
        ok, detail = fn()
    except Exception as e:  # any network or parse error is a red check, with its reason
        ok, detail = False, f"{type(e).__name__}: {e}"
    print(f"  {'ok  ' if ok else 'FAIL'} {name}" + (f" — {detail}" if detail else ""))
    if not ok:
        fails.append(name)

def api_build():
    _, _, b = fetch(W + "/api/version")
    got = json.loads(b).get("build")
    return got == sha, f"build={got}"

def page(base):
    def run():
        _, _, b = fetch(base + "/")
        html = b.decode("utf-8", "replace")
        if sha not in html:
            return False, "page does not carry the build id"
        chunk = re.search(r'(/_next/static/chunks/[^"\'\s]+\.js)', html)
        if not chunk:
            return False, "no JS chunk referenced"
        st, _, _ = fetch(base + chunk.group(1))
        return st == 200, f"chunk {chunk.group(1).rsplit('/', 1)[-1]} HTTP {st}"
    return run

def catalogue():
    _, _, b = fetch(W + "/api/events?past=true")
    d = json.loads(b)
    n = len(d if isinstance(d, list) else d.get("items", []))
    return n > 0, f"{n} events"

def organiser():
    _, _, b = fetch(W + "/api/auth/login",
                    json.dumps({"email": "maker@underground.dev", "password": os.environ["PW"]}).encode(),
                    {"content-type": "application/json"})
    tok = json.loads(b)["token"]
    _, _, b = fetch(W + "/api/me/events", headers={"authorization": "Bearer " + tok})
    d = json.loads(b)
    n = len(d if isinstance(d, list) else d.get("items", []))
    return n > 0, f"{n} own events"

def noindex():
    _, h, _ = fetch(W + "/")
    v = {k.lower(): val for k, val in h.items()}.get("x-robots-tag", "")
    return "noindex" in v.lower(), v or "header missing"

check(f"api serves build {sha[:12]}", api_build)
check("web serves this build and its JS loads", page(W))
check("admin serves this build and its JS loads", page(A))
check("catalogue is served", catalogue)
check("demo organiser signs in and sees own events", organiser)
check("DEV is noindex", noindex)
print(("SMOKE GREEN " if not fails else "SMOKE RED ") + sha)
sys.exit(1 if fails else 0)
PY
