#!/bin/bash
# Deploys site/out to brevi-lang.org and site/out-run to brevi.run as two Cloudflare Workers
# with the files embedded. Needs a Cloudflare API token (CF_API_TOKEN) with Workers Scripts Edit.
set -e
S=$(dirname "$(readlink -f "$0")"); bash $S/build.sh
ACCT=8cc87b4468e236cc48fe7e53cc9b8469
API=https://api.cloudflare.com/client/v4/accounts/$ACCT
AUTH="Authorization: Bearer ${CF_API_TOKEN:-proxy}"
T=$(mktemp -d)
deploy() { # name, dir, hostname, zone id
  python3 - "$2" > $T/$1.mjs <<'PY'
import json, os, sys
d = sys.argv[1]
import base64
types = {".html": "text/html; charset=utf-8", ".md": "text/markdown; charset=utf-8", ".txt": "text/plain; charset=utf-8",
         ".svg": "image/svg+xml", ".webmanifest": "application/manifest+json", ".png": "image/png", ".ico": "image/x-icon", ".woff2": "font/woff2"}
binary = {".png", ".ico", ".woff2"}
files = {}
for f in sorted(os.listdir(d)):
    ext = os.path.splitext(f)[1]
    raw = open(os.path.join(d, f), "rb").read()
    files["/" + f] = [base64.b64encode(raw).decode() if ext in binary else raw.decode(), types[ext], ext in binary]
print("const FILES = " + json.dumps(files) + ";")
print("const bytes = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));")
print('''export default {
  fetch(req) {
    let p = new URL(req.url).pathname;
    if (p.endsWith("/")) p += "index.html";
    const f = FILES[p];
    if (!f) return new Response("Not found\\n", { status: 404, headers: { "content-type": "text/plain" } });
    // Pages change with each deploy; fonts and icons almost never do.
    const cache = /\.(woff2|png|ico|svg)$/.test(p) ? "public, max-age=604800, stale-while-revalidate=86400" : "public, max-age=300, stale-while-revalidate=3600";
    return new Response(f[2] ? bytes(f[0]) : f[0], { headers: { "content-type": f[1], "cache-control": cache } });
  },
};''')
PY
  echo '{"main_module":"worker.mjs","compatibility_date":"2026-10-01"}' > $T/meta.json
  curl -sS -X PUT "$API/workers/scripts/$1" -H "$AUTH" \
    -F "metadata=@$T/meta.json;type=application/json" \
    -F "worker.mjs=@$T/$1.mjs;filename=worker.mjs;type=application/javascript+module" | python3 -c "import sys,json;d=json.load(sys.stdin);print('$1 upload', d['success'], d['errors'])"
  curl -sS -X PUT "$API/workers/domains" -H "$AUTH" -H "content-type: application/json" \
    -d "{\"hostname\":\"$3\",\"zone_id\":\"$4\",\"service\":\"$1\",\"environment\":\"production\"}" | python3 -c "import sys,json;d=json.load(sys.stdin);print('$3 domain', d['success'], d['errors'])"
}
deploy brevi-site $S/out brevi-lang.org 00f92f504990fac1ae44275ba2c1af45
deploy brevi-site $S/out www.brevi-lang.org 00f92f504990fac1ae44275ba2c1af45
deploy brevi-run-site $S/out-run brevi.run 0086633f00f8d7798f31e3b5abe30398
