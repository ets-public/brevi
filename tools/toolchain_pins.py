#!/usr/bin/env python3
"""Writes src/toolchain_pins.rs: the URL and SHA-256 of every file Brevi's pinned toolchain downloads.

When no Rust compiler is installed, brevi fetches Rust (the version in .github/rust-version, the one CI tests
with) and, where no working C linker is installed, zig, which links for it (src/toolchain.rs). The hashes are
compiled into brevi, so a download is checked against what this script saw, never against a file fetched
next to it.

usage: python3 tools/toolchain_pins.py          rewrite src/toolchain_pins.rs from the published manifests
       python3 tools/toolchain_pins.py --check  fail when the file's Rust version is not .github/rust-version
Run it whenever .github/rust-version or ZIG below changes.
"""
import json
import pathlib
import sys
import tomllib
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "src" / "toolchain_pins.rs"
ZIG = "0.17.1"
# Brevi's host -> (the Rust host triple of the pinned toolchain, Rust components, zig's name for the host).
# Windows uses the gnullvm toolchain: it needs no Visual Studio, and zig supplies the MinGW C runtime it links.
HOSTS = {
    "x86_64-unknown-linux-gnu": ("x86_64-unknown-linux-gnu", ["rustc", "rust-std", "cargo"], "x86_64-linux"),
    "aarch64-unknown-linux-gnu": ("aarch64-unknown-linux-gnu", ["rustc", "rust-std", "cargo"], "aarch64-linux"),
    "x86_64-apple-darwin": ("x86_64-apple-darwin", ["rustc", "rust-std", "cargo"], "x86_64-macos"),
    "aarch64-apple-darwin": ("aarch64-apple-darwin", ["rustc", "rust-std", "cargo"], "aarch64-macos"),
    "x86_64-pc-windows-msvc": ("x86_64-pc-windows-gnullvm", ["rustc", "rust-std", "rust-mingw", "cargo"], "x86_64-windows"),
    "aarch64-pc-windows-msvc": ("aarch64-pc-windows-gnullvm", ["rustc", "rust-std", "rust-mingw", "cargo"], "aarch64-windows"),
}


def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": "brevi-toolchain-pins"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def rust_version():
    return (ROOT / ".github" / "rust-version").read_text().strip()


def main():
    if sys.argv[1:] == ["--check"]:
        want = f'pub(crate) const RUST: &str = "{rust_version()}";'
        if want not in OUT.read_text():
            sys.exit(f"{OUT.relative_to(ROOT)} does not pin Rust {rust_version()} (.github/rust-version): run python3 tools/toolchain_pins.py")
        return
    rv = rust_version()
    pkg = tomllib.loads(fetch(f"https://static.rust-lang.org/dist/channel-rust-{rv}.toml").decode())["pkg"]
    zig = json.loads(fetch("https://ziglang.org/download/index.json"))[ZIG]
    lines = [
        "// Written by tools/toolchain_pins.py: do not edit by hand.",
        f'pub(crate) const RUST: &str = "{rv}";',
        f'pub(crate) const ZIG: &str = "{ZIG}";',
        "",
        "// (Brevi's host, the pinned toolchain's host, [(Rust component, URL, SHA-256)], (zig URL, SHA-256))",
        "#[allow(clippy::type_complexity)]",
        "pub(crate) const PINS: &[(&str, &str, &[(&str, &str, &str)], (&str, &str))] = &[",
    ]
    for host, (triple, comps, zhost) in HOSTS.items():
        parts = []
        for c in comps:
            t = pkg[c]["target"][triple]
            if not t.get("available"):
                sys.exit(f"Rust {rv} has no {c} for {triple}")
            parts.append(f'("{c}", "{t["url"]}", "{t["hash"]}")')
        z = zig[zhost]
        lines.append(f'    ("{host}", "{triple}", &[{", ".join(parts)}], ("{z["tarball"]}", "{z["shasum"]}")),')
    lines.append("];")
    OUT.write_text("\n".join(lines) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}: Rust {rv}, zig {ZIG}, {len(HOSTS)} hosts")


if __name__ == "__main__":
    main()
