#!/usr/bin/env python3
"""Checks that the Dockerfiles' base images are pinned, and pinned to what they say.

- Every FROM line in Dockerfile and templates/image/Dockerfile names its image by tag and digest.
- The rust images name .github/rust-version, and (unless --offline) the digest really is that Rust: the
  linux/amd64 image's config has RUST_VERSION set to it. Docker uses the digest and ignores the tag, so
  a tag bumped without its digest would otherwise build with the old Rust and say nothing.

usage: python3 tools/docker_pins.py [--offline]     (toolchains.yml runs it)
"""
import json
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILES = ["Dockerfile", "templates/image/Dockerfile"]
FROM = re.compile(r"^FROM\s+(\S+)", re.M)
PINNED = re.compile(r"^([a-z0-9._/-]+):([A-Za-z0-9._-]+)@(sha256:[0-9a-f]{64})$")
INDEX = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json"
MANIFEST = "application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json"


def get(url, accept=None, token=None):
    headers = {"User-Agent": "docker-pins-check"}
    if accept:
        headers["Accept"] = accept
    if token:
        headers["Authorization"] = f"Bearer {token}"
    for attempt in range(3):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=30) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            if e.code == 404 or attempt == 2:
                raise
            time.sleep(5)
        except OSError:
            if attempt == 2:
                raise
            time.sleep(5)


def rust_version_of(image, digest):
    repo = image if "/" in image else f"library/{image}"
    token = get(f"https://auth.docker.io/token?service=registry.docker.io&scope=repository:{repo}:pull")["token"]
    base = f"https://registry-1.docker.io/v2/{repo}"
    index = get(f"{base}/manifests/{digest}", INDEX + ", " + MANIFEST, token)
    if "manifests" in index:
        amd64 = [m["digest"] for m in index["manifests"]
                 if m.get("platform", {}).get("architecture") == "amd64" and m["platform"].get("os") == "linux"]
        if not amd64:
            return None
        index = get(f"{base}/manifests/{amd64[0]}", MANIFEST, token)
    config = get(f"{base}/blobs/{index['config']['digest']}", None, token)
    for env in config.get("config", {}).get("Env", []):
        if env.startswith("RUST_VERSION="):
            return env.split("=", 1)[1]
    return None


def main():
    offline = "--offline" in sys.argv[1:]
    pin = (ROOT / ".github/rust-version").read_text().strip()
    errors = []
    for name in FILES:
        for ref in FROM.findall((ROOT / name).read_text()):
            m = PINNED.match(ref)
            if not m:
                errors.append(f"{name}: FROM {ref} is not pinned as IMAGE:TAG@sha256:DIGEST")
                continue
            image, tag, digest = m.groups()
            if image.split("/")[-1] != "rust":
                continue
            if not tag.startswith(pin + "-") and tag != pin:
                errors.append(f"{name}: FROM {ref} is not Rust {pin} (.github/rust-version)")
            elif not offline:
                try:
                    found = rust_version_of(image, digest)
                except urllib.error.HTTPError as e:
                    found = f"unknown (registry answered {e.code})"
                if found != pin:
                    errors.append(f"{name}: the image {digest} is Rust {found}, not {pin}: update the digest with the tag")
    for e in errors:
        print(f"::error::{e}")
    if errors:
        sys.exit(1)
    print(f"docker base images pinned by digest; rust images are {pin}")


if __name__ == "__main__":
    main()
