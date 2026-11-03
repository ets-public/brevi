#!/bin/bash
# Deploys site/out to brevi-lang.org and site/out-run to brevi.run as two Cloudflare Workers
# with the files embedded. Needs a Cloudflare API token (CF_API_TOKEN) with Workers Scripts Edit.
# `bash site/deploy.sh --preview` uploads to the Workers brevi-site-preview and brevi-run-preview on the
# account's workers.dev subdomain instead, for review; the live sites are not touched.
set -e
S=$(dirname "$(readlink -f "$0")"); bash $S/build.sh
ACCT=8cc87b4468e236cc48fe7e53cc9b8469
API=https://api.cloudflare.com/client/v4/accounts/$ACCT
AUTH="Authorization: Bearer ${CF_API_TOKEN:-proxy}"
T=$(mktemp -d)
deploy_script() { # name, dir: build the Worker module and upload it
  python3 - "$2" > $T/$1.mjs <<'PY'
import json, os, sys
d = sys.argv[1]
import base64
types = {".html": "text/html; charset=utf-8", ".md": "text/markdown; charset=utf-8", ".txt": "text/plain; charset=utf-8",
         ".css": "text/css; charset=utf-8", ".svg": "image/svg+xml", ".webmanifest": "application/manifest+json",
         ".png": "image/png", ".ico": "image/x-icon", ".woff2": "font/woff2"}
binary = {".png", ".ico", ".woff2"}
files = {}
for top, _, names in sorted(os.walk(d)):
    for f in sorted(names):
        ext = os.path.splitext(f)[1]
        raw = open(os.path.join(top, f), "rb").read()
        key = "/" + os.path.relpath(os.path.join(top, f), d).replace(os.sep, "/")
        files[key] = [base64.b64encode(raw).decode() if ext in binary else raw.decode(), types[ext], ext in binary]
print("const FILES = " + json.dumps(files) + ";")
print("const bytes = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));")
print('''export default {
  fetch(req) {
    const url = new URL(req.url);
    let p = url.pathname;
    // Clean addresses: /docs/usage serves docs/usage.html; /docs (a folder) redirects to /docs/.
    if (!p.endsWith("/") && !FILES[p] && FILES[p + "/index.html"]) return Response.redirect(url.origin + p + "/" + url.search, 301);
    if (p.endsWith("/")) p += "index.html";
    else if (!FILES[p] && FILES[p + ".html"]) p += ".html";
    const f = FILES[p];
    if (!f) {
      const nf = FILES["/404.html"];
      return new Response(nf ? nf[0] : "Not found\\n", { status: 404, headers: { "content-type": nf ? nf[1] : "text/plain" } });
    }
    // Pages change with each deploy; fonts, icons and diagrams almost never do (style.css is versioned in its URL).
    const cache = /\\.(woff2|png|ico|svg|css)$/.test(p) ? "public, max-age=604800, stale-while-revalidate=86400" : "public, max-age=300, stale-while-revalidate=3600";
    return new Response(f[2] ? bytes(f[0]) : f[0], { headers: { "content-type": f[1], "cache-control": cache } });
  },
};''')
PY
  echo '{"main_module":"worker.mjs","compatibility_date":"2026-10-01"}' > $T/meta.json
  curl -sS -X PUT "$API/workers/scripts/$1" -H "$AUTH" \
    -F "metadata=@$T/meta.json;type=application/json" \
    -F "worker.mjs=@$T/$1.mjs;filename=worker.mjs;type=application/javascript+module" | python3 -c "import sys,json;d=json.load(sys.stdin);print('$1 upload', d['success'], d['errors'])"
}
deploy() { # name, dir, hostname, zone id
  deploy_script "$1" "$2"
  curl -sS -X PUT "$API/workers/domains" -H "$AUTH" -H "content-type: application/json" \
    -d "{\"hostname\":\"$3\",\"zone_id\":\"$4\",\"service\":\"$1\",\"environment\":\"production\"}" | python3 -c "import sys,json;d=json.load(sys.stdin);print('$3 domain', d['success'], d['errors'])"
}
if [ "$1" = --preview ]; then
  preview() { # name, dir: upload, then serve it on workers.dev
    deploy_script "$1" "$2"
    curl -sS -X POST "$API/workers/scripts/$1/subdomain" -H "$AUTH" -H "content-type: application/json" -d '{"enabled":true}' >/dev/null
    sub=$(curl -sS "$API/workers/subdomain" -H "$AUTH" | python3 -c "import sys,json;print(json.load(sys.stdin)['result']['subdomain'])")
    echo "preview: https://$1.$sub.workers.dev/"
  }
  preview brevi-site-preview $S/out
  preview brevi-run-preview $S/out-run
  exit 0
fi
deploy brevi-site $S/out brevi-lang.org 00f92f504990fac1ae44275ba2c1af45
deploy brevi-site $S/out www.brevi-lang.org 00f92f504990fac1ae44275ba2c1af45
deploy brevi-run-site $S/out-run brevi.run 0086633f00f8d7798f31e3b5abe30398
