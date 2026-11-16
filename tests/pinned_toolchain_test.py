#!/usr/bin/env python3
"""Brevi's pinned toolchain (src/toolchain.rs) on this machine: no system Rust, zig as the linker.

usage: BREVI=path/to/brevi python3 tests/pinned_toolchain_test.py
Run by .github/workflows/pinned-toolchain.yml on every platform brevi ships for; a platform goes into
toolchain.rs's VERIFIED list only after this and that platform's usual checks pass there with
BREVI_TOOLCHAIN=pinned and BREVI_LINKER=zig.

It fetches the toolchain into a fresh folder, then builds and runs, checked against CPython, a typed program, a
dynamic one, a standard-library one and a --release build, and checks they were built by the pinned rustc and
linked by zig.
"""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

BREVI = os.path.abspath(os.environ.get("BREVI", "target/release/brevi"))
PROGRAMS = {
    "typed.py": ("def f(n: int) -> int:\n    return sum(i * i for i in range(n))\nprint(f(1000), 'typed')\n", "332833500 typed\n"),
    "dynamic.py": (
        "class A:\n    def __init__(s, x): s.x = x\n    def __repr__(s): return f'A({s.x!r})'\n"
        "print([A(i) for i in range(3)], getattr(A(5), 'x'))\n",
        "[A(0), A(1), A(2)] 5\n",
    ),
    "stdlib.py": (
        "import json, re, collections\nc = collections.Counter('hello world')\n"
        "print(json.dumps(sorted(c.items())[:3]), re.findall(r'o\\w', 'foo bow'))\n",
        '[[" ", 1], ["d", 1], ["e", 1]] [\'oo\', \'ow\']\n',
    ),
}


def run(args, env, cwd, check=True):
    p = subprocess.run([BREVI, *args], env=env, cwd=cwd, capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=1800)
    if check and p.returncode != 0:
        sys.exit(f"FAIL: brevi {' '.join(args)} exited {p.returncode}\nstdout: {p.stdout}\nstderr: {p.stderr}")
    return p


def main():
    work = pathlib.Path(tempfile.mkdtemp(prefix="brevi-pinned-"))
    env = dict(os.environ)
    env.update(BREVI_TOOLCHAIN="pinned", BREVI_LINKER="zig", BREVI_TOOLCHAIN_DIR=str(work / "tc"), BREVI_CACHE=str(work / "cache"))
    fails = 0
    try:
        info = json.loads(run(["toolchain"], env, work).stdout.strip().splitlines()[-1])
        print("toolchain:", json.dumps(info))
        tc = str(work / "tc")
        if not (info.get("pinned") and info["rustc"].startswith(tc) and "brevi-cc" in info.get("linker", "")):
            sys.exit("FAIL: brevi toolchain does not report the pinned rustc and zig's brevi-cc")
        rv = pathlib.Path(__file__).resolve().parent.parent.joinpath(".github", "rust-version").read_text().strip()
        if f"rustc {rv} " not in info["version"] + " ":
            sys.exit(f"FAIL: pinned rustc is {info['version']}, not {rv}")
        for name, (src, want) in PROGRAMS.items():
            (work / name).write_text(src)
            p = run(["run", "--verify", name], env, work, check=False)
            # The program's output on stdout; brevi's JSON lines (the path taken, the certificate) on stderr.
            cert = next((json.loads(l) for l in p.stderr.splitlines() if l.startswith('{"verify"')), {})
            out = p.stdout
            ok = p.returncode == 0 and cert.get("verify") == "identical" and out == want
            print(("ok  " if ok else "FAIL"), name, cert.get("path"), json.dumps(out))
            if not ok:
                print(p.stdout, p.stderr, sep="\n")
                fails += 1
        exe = work / ("typed_release" + (".exe" if os.name == "nt" else ""))
        run(["build", "typed.py", "--release", "-o", str(exe)], env, work)
        got = subprocess.run([str(exe)], capture_output=True, text=True, timeout=60).stdout
        ok = got == PROGRAMS["typed.py"][1]
        print(("ok  " if ok else "FAIL"), "build --release", json.dumps(got))
        fails += not ok
    finally:
        shutil.rmtree(work, ignore_errors=True)
    if fails:
        sys.exit(f"{fails} pinned-toolchain check(s) failed")
    print("pinned toolchain: all checks passed")


if __name__ == "__main__":
    main()
