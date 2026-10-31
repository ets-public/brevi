"""Fails when a packaging manifest's version differs from Cargo.toml. usage: version_sync.py"""
import json, os, re, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
cargo = re.search(r'^version = "([^"]+)"', open(os.path.join(ROOT, "Cargo.toml")).read(), re.M)[1]
found = {
    "plugins/brevi/.claude-plugin/plugin.json": json.load(open(os.path.join(ROOT, "plugins/brevi/.claude-plugin/plugin.json")))["version"],
    "packaging/npm/package.json": json.load(open(os.path.join(ROOT, "packaging/npm/package.json")))["version"],
}
srv = json.load(open(os.path.join(ROOT, "server.json")))
found["server.json"] = srv["version"]
for i, p in enumerate(srv.get("packages", [])):
    found[f"server.json packages[{i}]"] = p["version"]
bad = {k: v for k, v in found.items() if v != cargo}
for k, v in bad.items():
    print(f"{k}: {v}, Cargo.toml has {cargo}")
print("versions match Cargo.toml" if not bad else "update these to match Cargo.toml")
sys.exit(1 if bad else 0)
