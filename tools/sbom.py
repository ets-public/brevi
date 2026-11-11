#!/usr/bin/env python3
"""Writes the release's software bill of materials: CycloneDX 1.5 JSON (brevi-sbom.cdx.json).

    python3 tools/sbom.py --tag v0.28.0 --dist dist > dist/brevi-sbom.cdx.json

What a brevi binary is made of: Brevi's own source (Cargo.lock lists every crate it is built from; today
that is brevi alone, with no outside crates) and the Rust standard library of the rustc that built it. Each
release archive in DIST is listed as a file with its SHA-256, so the SBOM names exactly what was shipped.
The output depends only on its inputs (no clock, no random id), so the same release gives the same file.
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import uuid

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPO = "https://github.com/ets-public/brevi"


def lock_packages(path):
    text = open(path, encoding="utf-8").read()
    out = []
    for block in text.split("[[package]]")[1:]:
        name = re.search(r'^name = "([^"]+)"', block, re.M)
        ver = re.search(r'^version = "([^"]+)"', block, re.M)
        src = re.search(r'^source = "([^"]+)"', block, re.M)
        if name and ver:
            out.append((name[1], ver[1], src[1] if src else None))
    return out


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def rustc_version():
    try:
        out = subprocess.run(["rustc", "--version"], capture_output=True, text=True, check=True).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    m = re.match(r"rustc (\S+)", out)
    return m[1] if m else None


def build(tag, dist, rustc):
    version = tag.lstrip("v")
    cargo = open(os.path.join(ROOT, "Cargo.toml"), encoding="utf-8").read()
    have = re.search(r'^version = "([^"]+)"', cargo, re.M)[1]
    if have != version:
        sys.exit(f"sbom: the tag says {version} but Cargo.toml says {have}")
    me = {
        "type": "application",
        "bom-ref": f"pkg:cargo/brevi@{version}",
        "name": "brevi",
        "version": version,
        "purl": f"pkg:cargo/brevi@{version}",
        "licenses": [{"license": {"id": "Apache-2.0"}}],
        "externalReferences": [{"type": "vcs", "url": REPO}, {"type": "distribution", "url": f"{REPO}/releases/tag/{tag}"}],
    }
    components = []
    deps = []
    for name, ver, src in lock_packages(os.path.join(ROOT, "Cargo.lock")):
        if name == "brevi":
            continue
        ref = f"pkg:cargo/{name}@{ver}"
        c = {"type": "library", "bom-ref": ref, "name": name, "version": ver, "purl": ref}
        if src:
            c["externalReferences"] = [{"type": "distribution", "url": src}]
        components.append(c)
        deps.append(ref)
    if rustc:
        ref = f"pkg:generic/rust-std@{rustc}"
        components.append({"type": "library", "bom-ref": ref, "name": "rust-std", "version": rustc, "purl": ref, "description": "The Rust standard library, linked into the binary by the rustc that built it", "licenses": [{"expression": "MIT OR Apache-2.0"}]})
        deps.append(ref)
    for f in sorted(os.listdir(dist)):
        p = os.path.join(dist, f)
        if not os.path.isfile(p) or not f.startswith("brevi-") or f.endswith(".cdx.json") or f.endswith(".intoto.jsonl"):
            continue
        components.append({"type": "file", "bom-ref": f"file:{f}", "name": f, "version": version, "hashes": [{"alg": "SHA-256", "content": sha256(p)}]})
    bom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{uuid.uuid5(uuid.NAMESPACE_URL, f'{REPO}/releases/tag/{tag}#sbom')}",
        "version": 1,
        "metadata": {"component": me, "tools": {"components": [{"type": "application", "name": "tools/sbom.py", "version": version}]}},
        "components": components,
        "dependencies": [{"ref": me["bom-ref"], "dependsOn": deps}] + [{"ref": d, "dependsOn": []} for d in deps],
    }
    return bom


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--tag", required=True)
    ap.add_argument("--dist", required=True, help="folder holding the release archives")
    ap.add_argument("--rustc", help="rustc version that built the binaries (default: rustc --version)")
    a = ap.parse_args()
    rustc = a.rustc or rustc_version()
    if not rustc:
        sys.exit("sbom: no rustc version (pass --rustc)")
    json.dump(build(a.tag, a.dist, rustc), sys.stdout, indent=2, sort_keys=True)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
