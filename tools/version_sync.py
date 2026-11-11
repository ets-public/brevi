"""Fails when a packaging manifest's version differs from Cargo.toml, or a package the release checks install
(tests/requirements-native.txt) differs from the version its tests were made with (tools/upstream_pins.json).
usage: version_sync.py [--tag vX.Y.Z]   (--tag: also fail unless the release tag is v + that version)"""
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
if "--tag" in sys.argv:
    found["release tag"] = sys.argv[sys.argv.index("--tag") + 1].removeprefix("v")
bad = {k: v for k, v in found.items() if v != cargo}
for k, v in bad.items():
    print(f"{k}: {v}, Cargo.toml has {cargo}")
# The packages the release checks install (tests/requirements-native.txt) must be the versions the .expected files
# were made with (tools/upstream_pins.json), or a release would compare Brevi's copies with another version.
pins = {}
for g in json.load(open(os.path.join(ROOT, "tools/upstream_pins.json")))["groups"].values():
    pins.update({k.lower(): v for k, v in {**g["watch"], **g.get("also", {})}.items()})
for m in re.finditer(r"^([A-Za-z0-9_.-]+)==(\S+)", open(os.path.join(ROOT, "tests/requirements-native.txt")).read(), re.M):
    name, v = m.group(1).lower(), m.group(2)
    if name in pins and pins[name] != v:
        bad[f"tests/requirements-native.txt {name}"] = v
        print(f"tests/requirements-native.txt: {name}=={v}, tools/upstream_pins.json has {pins[name]}")
print("versions match Cargo.toml" if not bad else "update these to match Cargo.toml")
sys.exit(1 if bad else 0)
