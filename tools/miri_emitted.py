#!/usr/bin/env python3
"""Run typed Brevi builds under Miri, to catch undefined behaviour in the Rust the compiler writes.

    python3 tools/miri_emitted.py [--brevi PATH] [--jobs N] FILE.py...

Each file is turned into Rust with `brevi emit` (BREVI_DYN=0 BREVI_GUARD=off, as tests/typed runs it) and run
with `cargo +nightly miri run`, fed FILE.in on stdin when there is one. Needs the nightly toolchain with the
miri component (`rustup toolchain install nightly --component miri,rust-src`). Prints one line per file:
OK, UB (with the line of generated code Miri points at), ERR (rustc refused the program: such a program goes
to the dynamic build, so it is not a finding) or TIMEOUT. Exits 1 when any file shows UB.

Miri runs with -Zmiri-permissive-provenance (the runtime's allocator finds a page from a block's address) and
-Zmiri-disable-isolation (programs read the clock and files). Miri is slow: a test program takes 10 s to a few
minutes. This is a review tool, not a CI step; docs/SECURITY_TESTING.md records the runs.
"""
import argparse
import concurrent.futures
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def run_one(brevi, path, work, timeout):
    path = os.path.abspath(path)
    d = os.path.join(work, re.sub(r"[^A-Za-z0-9]", "_", os.path.relpath(path, ROOT)))
    os.makedirs(os.path.join(d, "src"), exist_ok=True)
    env = dict(os.environ, BREVI_DYN="0", BREVI_GUARD="off")
    em = subprocess.run([brevi, "emit", os.path.basename(path)], cwd=os.path.dirname(path), env=env, capture_output=True, text=True)
    if em.returncode != 0:
        return "NOEMIT", path, em.stderr.strip()[:100]
    with open(os.path.join(d, "src", "main.rs"), "w") as f:
        f.write(em.stdout)
    with open(os.path.join(d, "Cargo.toml"), "w") as f:
        f.write('[package]\nname = "m"\nversion = "0.1.0"\nedition = "2021"\n[dependencies]\n[workspace]\n')
    inp = os.path.splitext(path)[0] + ".in"
    menv = dict(os.environ, MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-permissive-provenance", CARGO_TARGET_DIR=os.path.join(d, "target"))
    try:
        with open(inp if os.path.exists(inp) else os.devnull) as stdin:
            r = subprocess.run(["cargo", "+nightly", "miri", "run", "-q"], cwd=d, env=menv, stdin=stdin, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return "TIMEOUT", path, ""
    if "Undefined Behavior" in r.stderr:
        tail = r.stderr[r.stderr.index("Undefined Behavior"):]
        m = re.search(r"src/main\.rs:(\d+)", tail)
        line = ""
        if m:
            src = em.stdout.splitlines()
            n = int(m.group(1))
            line = src[n - 1].strip()[:160] if n <= len(src) else ""
        return "UB", path, line
    if re.search(r"^error", r.stderr, re.M):
        return "ERR", path, re.search(r"^error.*$", r.stderr, re.M).group(0)[:100]
    return "OK", path, ""


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--brevi", default=os.path.join(ROOT, "target", "release", "brevi"))
    ap.add_argument("--jobs", type=int, default=max(1, (os.cpu_count() or 2) // 2))
    ap.add_argument("--timeout", type=int, default=600)
    ap.add_argument("files", nargs="+")
    a = ap.parse_args()
    bad = 0
    with tempfile.TemporaryDirectory(prefix="miri_emitted_") as work:
        with concurrent.futures.ThreadPoolExecutor(a.jobs) as ex:
            for st, path, detail in ex.map(lambda p: run_one(a.brevi, p, work, a.timeout), a.files):
                print(f"{st:7} {os.path.relpath(path, ROOT)}" + (f"  {detail}" if detail else ""), flush=True)
                bad += st == "UB"
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
