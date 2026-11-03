#!/bin/bash
# Prints the /pricing waitlist sign-ups as CSV (list, email, note, signed up, country), oldest first.
# `bash site/waitlist.sh --preview` reads the preview site's list instead. Needs CF_API_TOKEN like deploy.sh.
set -e
ACCT=8cc87b4468e236cc48fe7e53cc9b8469
API=https://api.cloudflare.com/client/v4/accounts/$ACCT
AUTH="Authorization: Bearer ${CF_API_TOKEN:-proxy}"
TITLE=brevi-waitlist; [ "$1" = --preview ] && TITLE=brevi-waitlist-preview
python3 - "$API" "$AUTH" "$TITLE" <<'PY'
import csv, json, sys, urllib.request, urllib.parse
api, auth, title = sys.argv[1:4]
def get(path, raw=False):
    req = urllib.request.Request(api + path, headers={auth.split(": ")[0]: auth.split(": ", 1)[1]})
    body = urllib.request.urlopen(req).read()
    return body if raw else json.loads(body)
ns = next((n["id"] for n in get("/storage/kv/namespaces?per_page=100")["result"] if n["title"] == title), None)
if not ns:
    sys.exit("no %s namespace yet (nothing deployed, or nobody has signed up)" % title)
keys, cursor = [], ""
while True:
    r = get("/storage/kv/namespaces/%s/keys?limit=1000%s" % (ns, "&cursor=" + cursor if cursor else ""))
    keys += [k["name"] for k in r["result"]]
    cursor = r.get("result_info", {}).get("cursor") or ""
    if not cursor:
        break
rows = [json.loads(get("/storage/kv/namespaces/%s/values/%s" % (ns, urllib.parse.quote(k, safe="")), raw=True)) for k in keys]
rows.sort(key=lambda x: x.get("ts", ""))
w = csv.writer(sys.stdout)
w.writerow(["list", "email", "note", "signed_up", "country"])
for x in rows:
    w.writerow([x.get("list"), x.get("email"), x.get("note"), x.get("ts"), x.get("country")])
print("%d sign-ups: %s" % (len(rows), ", ".join("%s %d" % (l, sum(1 for x in rows if x.get("list") == l)) for l in sorted({x.get("list") for x in rows}))), file=sys.stderr)
PY
