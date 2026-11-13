#!/usr/bin/env bash
# DEV smoke, run ON THE BOX by deploy-dev.sh through the public URLs (Caddy + TLS, as a visitor).
# Usage: bash smoke-dev.sh <commit>
#
# Proves: the served API, web and admin builds are <commit> (a stale container cannot pass); each page's own
# JS chunk loads (a page whose scripts 404 is dead even when it answers 200); the catalogue is served; a demo
# organiser signs in and lists its events; DEV is marked noindex; web, admin, API and /uploads answers carry
# X-Frame-Options: DENY and X-Content-Type-Options: nosniff (TASK-305 F5). Prints pass/fail only: passwords
# and tokens stay inside this process and never reach a command line or the output.
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

def page(base, must=()):
    def run():
        _, _, b = fetch(base + "/")
        html = b.decode("utf-8", "replace")
        if sha not in html:
            return False, "page does not carry the build id"
        # The page must be the real home: a full document with the site shell, not just "some 200".
        for needle in ("<html",) + tuple(must):
            if needle not in html:
                return False, f"home lacks {needle!r} (wrong page or no layout)"
        chunk = re.search(r'(/_next/static/chunks/[^"\'\s]+\.js)', html)
        css = re.search(r'(/_next/static/css/[^"\'\s]+\.css)', html)
        if not chunk or not css:
            return False, "no JS chunk or stylesheet referenced"
        st, _, _ = fetch(base + chunk.group(1))
        st2, _, _ = fetch(base + css.group(1))
        return st == 200 and st2 == 200, f"chunk HTTP {st}, stylesheet HTTP {st2}"
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

def lower_headers(url):
    """Headers of a GET, from an error answer too (a missing /uploads file is a 404 with headers)."""
    try:
        _, h, _ = fetch(url)
    except urllib.error.HTTPError as e:
        h = dict(e.headers)
    return {k.lower(): v for k, v in h.items()}

def uploads_url():
    """A real /uploads file from the catalogue when one exists, else a missing one (same layer serves both)."""
    _, _, b = fetch(W + "/api/events?past=true")
    d = json.loads(b)
    for e in (d if isinstance(d, list) else d.get("items", [])):
        for u in [e.get("poster_url")] + list(e.get("gallery_urls") or []):
            if isinstance(u, str) and u.startswith("/uploads/"):
                return W + u
    return W + "/uploads/smoke-missing.webp"

def security_headers(url_fn):
    """TASK-305 F5: the response must refuse framing and MIME sniffing."""
    def run():
        url = url_fn()
        h = lower_headers(url)
        got = (h.get("x-frame-options", ""), h.get("x-content-type-options", ""))
        ok = got[0].upper() == "DENY" and got[1].lower() == "nosniff"
        return ok, f"{url.replace(W, '').replace(A, '') or '/'}: x-frame-options={got[0]!r} nosniff={got[1]!r}"
    return run

check(f"api serves build {sha[:12]}", api_build)
check("web home is the real page (shell, styles, JS) of this build", page(W, ("class=\"topbar\"", "<title>Underground")))
check("admin serves this build with styles and JS", page(A))
check("catalogue is served", catalogue)
check("demo organiser signs in and sees own events", organiser)
check("DEV is noindex", noindex)
check("web refuses framing and sniffing", security_headers(lambda: W + "/"))
check("admin refuses framing and sniffing", security_headers(lambda: A + "/"))
check("api refuses framing and sniffing", security_headers(lambda: W + "/api/version"))
check("uploads refuse framing and sniffing", security_headers(uploads_url))
print(("SMOKE GREEN " if not fails else "SMOKE RED ") + sha)
sys.exit(1 if fails else 0)
PY
