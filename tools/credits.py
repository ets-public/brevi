"""Checks that every outside contributor whose PR landed since a tag is thanked in CHANGELOG.md's Unreleased
section or a changelog.d/ fragment ("thanks @handle"), so each release credits them. Run before a release; exits 1 naming who is missing.
Asks GitHub (gh api) who wrote each commit; the owner and bots need no credit.
usage: python3 tools/credits.py v0.24.6"""
import json, os, re, subprocess, sys

REPO = "ets-public/brevi"
OWNERS = {"ets-public"}

tag = sys.argv[1]
root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
sys.path.insert(0, os.path.join(root, "tools"))
import changelog
unreleased = changelog.unreleased()  # CHANGELOG.md's Unreleased lines plus changelog.d/ fragments
out = subprocess.run(["gh", "api", "--paginate", f"repos/{REPO}/compare/{tag}...main", "--jq", ".commits[]"],
                     capture_output=True, text=True, check=True).stdout
missing = {}
for line in out.splitlines():
    c = json.loads(line)
    login = (c.get("author") or {}).get("login") or c["commit"]["author"]["name"]
    if login in OWNERS or login.endswith("[bot]"):
        continue
    pr = re.search(r"\(#(\d+)\)", c["commit"]["message"].split("\n")[0])
    if f"thanks @{login}" not in unreleased:
        missing.setdefault(login, []).append("#" + pr.group(1) if pr else c["sha"][:8])
for login, prs in missing.items():
    print(f"not credited: @{login} ({', '.join(prs)}): end their changelog.d/ entries with ([#N](...), thanks @{login})")
if not missing:
    print("every outside contributor since " + tag + " is credited")
sys.exit(1 if missing else 0)
