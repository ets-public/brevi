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
kv_id() { # title: the id of the KV namespace with that title, creating it if needed
  local id
  id=$(curl -sS "$API/storage/kv/namespaces?per_page=100" -H "$AUTH" | python3 -c "import sys,json;print(next((n['id'] for n in json.load(sys.stdin)['result'] if n['title']==sys.argv[1]),''))" "$1")
  [ -n "$id" ] || id=$(curl -sS -X POST "$API/storage/kv/namespaces" -H "$AUTH" -H "content-type: application/json" -d "{\"title\":\"$1\"}" | python3 -c "import sys,json;print(json.load(sys.stdin)['result']['id'])")
  echo "$id"
}
deploy_script() { # name, dir[, KV namespace title for the waitlist]: build the Worker module and upload it
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
print('''// Waitlist sign-ups from /pricing (POST /api/waitlist), kept in the WAITLIST KV namespace as
// "LIST:email" -> {email, list, note, ts, country}. No IP address is stored. site/waitlist.sh exports them.
const LISTS = new Set(["pro", "hosted"]);
async function waitlist(req, env, url) {
  if (!env.WAITLIST) return new Response("Waitlist not available here\\n", { status: 404 });
  let form;
  try { form = await req.formData(); } catch { return new Response("Bad form\\n", { status: 400 }); }
  const list = String(form.get("list") || "");
  const email = String(form.get("email") || "").trim().toLowerCase();
  const note = String(form.get("note") || "").trim().slice(0, 500);
  const back = (q) => Response.redirect(url.origin + "/pricing" + q, 303);
  if (!LISTS.has(list)) return new Response("Unknown list\\n", { status: 400 });
  // A bot that fills the hidden field is told it worked and nothing is kept.
  if (form.get("website")) return back("?joined=" + list + "#" + list);
  if (email.length > 254 || !/^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$/.test(email))
    return new Response("Please go back and enter a valid email address.\\n", { status: 400, headers: { "content-type": "text/plain; charset=utf-8" } });
  const key = list + ":" + email;
  const old = await env.WAITLIST.get(key, "json");
  const rec = { email, list, note: note || (old && old.note) || "", ts: (old && old.ts) || new Date().toISOString(), country: (req.cf && req.cf.country) || "" };
  if (old) rec.updated = new Date().toISOString();
  await env.WAITLIST.put(key, JSON.stringify(rec));
  return back("?joined=" + list + "#" + list);
}
export default {
  async fetch(req, env) {
    const url = new URL(req.url);
    if (url.pathname === "/api/waitlist") {
      if (req.method !== "POST") return new Response("Use the form on /pricing\\n", { status: 405, headers: { allow: "POST" } });
      return waitlist(req, env || {}, url);
    }
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
  # A third argument names a KV namespace to bind as WAITLIST (created on first deploy).
  if [ -n "$3" ]; then
    kv=$(kv_id "$3")
    echo "{\"main_module\":\"worker.mjs\",\"compatibility_date\":\"2026-10-01\",\"bindings\":[{\"type\":\"kv_namespace\",\"name\":\"WAITLIST\",\"namespace_id\":\"$kv\"}]}" > $T/meta.json
  else
    echo '{"main_module":"worker.mjs","compatibility_date":"2026-10-01"}' > $T/meta.json
  fi
  curl -sS -X PUT "$API/workers/scripts/$1" -H "$AUTH" \
    -F "metadata=@$T/meta.json;type=application/json" \
    -F "worker.mjs=@$T/$1.mjs;filename=worker.mjs;type=application/javascript+module" | python3 -c "import sys,json;d=json.load(sys.stdin);print('$1 upload', d['success'], d['errors'])"
}
deploy() { # name, dir, hostname, zone id[, KV namespace title]
  deploy_script "$1" "$2" "$5"
  curl -sS -X PUT "$API/workers/domains" -H "$AUTH" -H "content-type: application/json" \
    -d "{\"hostname\":\"$3\",\"zone_id\":\"$4\",\"service\":\"$1\",\"environment\":\"production\"}" | python3 -c "import sys,json;d=json.load(sys.stdin);print('$3 domain', d['success'], d['errors'])"
}
if [ "$1" = --preview ]; then
  preview() { # name, dir[, KV namespace title]: upload, then serve it on workers.dev
    deploy_script "$1" "$2" "$3"
    curl -sS -X POST "$API/workers/scripts/$1/subdomain" -H "$AUTH" -H "content-type: application/json" -d '{"enabled":true}' >/dev/null
    sub=$(curl -sS "$API/workers/subdomain" -H "$AUTH" | python3 -c "import sys,json;print(json.load(sys.stdin)['result']['subdomain'])")
    echo "preview: https://$1.$sub.workers.dev/"
  }
  preview brevi-site-preview $S/out brevi-waitlist-preview
  preview brevi-run-preview $S/out-run
  exit 0
fi
deploy brevi-site $S/out brevi-lang.org 00f92f504990fac1ae44275ba2c1af45 brevi-waitlist
deploy brevi-site $S/out www.brevi-lang.org 00f92f504990fac1ae44275ba2c1af45 brevi-waitlist
deploy brevi-run-site $S/out-run brevi.run 0086633f00f8d7798f31e3b5abe30398
