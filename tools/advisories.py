#!/usr/bin/env python3
"""Looks up security advisories (OSV: RustSec, GitHub advisories, PyPA, npm) for everything Brevi depends on.

Covers the crates in Cargo.lock and runtime/brevi_tk, crates Brevi programs pin with `use NAME "VERSION"`, the
Python libraries Brevi mirrors (tools/upstream_pins.json) and the Python tools CI installs (tools/requirements-ci.txt).
Prints a Markdown summary; with --issue, opens one `security` issue per advisory (deduplicated by title).
No account or install needed: it uses the free https://osv.dev API.

usage: advisories.py [--issue] [--json]
"""
import json, re, sys, urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import upstream_watch as uw  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def inventory():
    deps = []  # (ecosystem, name, version, where)
    lock = (ROOT / "Cargo.lock").read_text()
    for m in re.finditer(r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"\nsource', lock):
        deps.append(("crates.io", m.group(1), m.group(2), "Cargo.lock"))
    tk = (ROOT / "runtime/brevi_tk/Cargo.toml").read_text()
    for m in re.finditer(r'^(\w[\w-]*)\s*=\s*\{\s*version\s*=\s*"([^"]+)"', tk, re.M):
        deps.append(("crates.io", m.group(1), m.group(2) + ".0" * (2 - m.group(2).count(".")), "runtime/brevi_tk"))
    for f in list(ROOT.glob("tests/**/*.bv")) + list(ROOT.glob("examples/**/*.bv")):
        for m in re.finditer(r'^\s*use\s+([A-Za-z0-9_-]+)\s+"([0-9][^"]*)"', f.read_text(), re.M):
            v = m.group(2)
            deps.append(("crates.io", m.group(1), v + ".0" * (2 - v.count(".")), "use in .bv"))
    groups = json.loads((ROOT / "tools/upstream_pins.json").read_text())["groups"]
    for g in groups.values():
        for p, v in {**g["watch"], **g.get("also", {})}.items():
            deps.append(("PyPI", p, v, "mirrored (tools/upstream_pins.json)"))
    req = ROOT / "tools/requirements-ci.txt"
    if req.exists():
        for m in re.finditer(r"^([A-Za-z0-9_.-]+)==([^\s#]+)", req.read_text(), re.M):
            deps.append(("PyPI", m.group(1), m.group(2), "tools/requirements-ci.txt"))
    return sorted(set(deps))


def query(deps):
    body = json.dumps({"queries": [{"package": {"name": n, "ecosystem": e}, "version": v} for e, n, v, _ in deps]}).encode()
    req = urllib.request.Request("https://api.osv.dev/v1/querybatch", body, {"Content-Type": "application/json", "User-Agent": "brevi-advisories"})
    with urllib.request.urlopen(req, timeout=60) as r:
        results = json.load(r)["results"]
    hits = []
    for d, res in zip(deps, results):
        seen = set()
        for v in res.get("vulns", []):
            if v["id"] in seen:
                continue
            full = uw.get(f"https://api.osv.dev/v1/vulns/{v['id']}")
            sev = (full.get("database_specific") or {}).get("severity") or ", ".join(s.get("score", "") for s in full.get("severity", [])) or "unrated"
            seen.update([v["id"], *full.get("aliases", [])])
            hits.append({"dep": d, "id": v["id"], "aliases": full.get("aliases", []), "summary": full.get("summary", ""), "severity": sev})
    return hits


if __name__ == "__main__":
    deps = inventory()
    hits = query(deps)
    if "--json" in sys.argv:
        print(json.dumps({"checked": len(deps), "advisories": hits}, indent=1))
        sys.exit(0)
    print(f"Checked {len(deps)} dependencies against OSV: {len(hits)} advisories.")
    for h in hits:
        e, n, v, where = h["dep"]
        print(f"- {h['id']} ({', '.join(h['aliases'])}) {n} {v} [{where}] {h['severity']}: {h['summary']}")
        if "--issue" in sys.argv:
            uw.LABEL = "security"
            uw.open_issue(f"Security: {h['id']} in {n} {v}", f"""OSV reports **{h['id']}** ({', '.join(h['aliases']) or 'no aliases'}) for {e} package `{n}` {v}, used in {where}.
Severity: {h['severity']}. {h['summary']}

https://osv.dev/vulnerability/{h['id']}

- [ ] Move to a fixed version (Cargo.lock / Cargo.toml / `use` line / tools/upstream_pins.json / tools/requirements-ci.txt)
- [ ] If Brevi mirrors this library natively, check whether Brevi's own copy has the same flaw
- [ ] Note the fix in CHANGELOG (Unreleased) under Security
""")
