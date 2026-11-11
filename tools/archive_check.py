#!/usr/bin/env python3
"""Checks a release archive the way a user gets it: unpacks it into an empty folder and runs the brevi inside.

Fails unless the archive holds brevi (brevi.exe on Windows), LICENSE, README.md and BREVI.md (and the brevi-python
link outside Windows), `brevi --version` names the release tag's version, and the unpacked brevi builds and runs a
typed .bv program and a Python program on the dynamic build (tests/dyn/classes.py, against its .expected file).
Prints the unpacked brevi's path last, so a later step can run more checks with it (release.yml runs
tests/windows/check.ps1 with it on Windows).

usage: archive_check.py ARCHIVE TAG [--arch x86_64]   (--arch: run it through `arch -x86_64`, for the Intel Mac
                                                         binary on an Apple silicon runner, under Rosetta)
Run from the repository root.
"""
import os, re, subprocess, sys, tarfile, tempfile, zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def fail(msg):
    print(f"FAIL {msg}")
    sys.exit(1)


def main():
    args = sys.argv[1:]
    arch = None
    if "--arch" in args:
        i = args.index("--arch")
        arch = args[i + 1]
        del args[i:i + 2]
    if len(args) != 2:
        sys.exit(__doc__)
    archive, tag = os.path.abspath(args[0]), args[1]
    version = tag.removeprefix("v")
    name = re.sub(r"\.(zip|tar\.gz)$", "", os.path.basename(archive))
    out = tempfile.mkdtemp(prefix="brevi-archive-")
    if archive.endswith(".zip"):
        with zipfile.ZipFile(archive) as z:
            z.extractall(out)
    else:
        with tarfile.open(archive) as t:
            t.extractall(out, filter="tar")
    top = os.listdir(out)
    if top != [name]:
        fail(f"{archive} unpacks to {top}, not one folder {name}")
    d = os.path.join(out, name)
    windows = name.endswith("windows-msvc")
    exe = os.path.join(d, "brevi.exe" if windows else "brevi")
    want = ["brevi.exe" if windows else "brevi", "LICENSE", "README.md", "BREVI.md"] + ([] if windows else ["brevi-python"])
    have = sorted(os.listdir(d))
    if sorted(want) != have:
        fail(f"{name} holds {have}, expected {sorted(want)}")
    pre = ["arch", f"-{arch}"] if arch else []

    def run(*a, stdin=None):
        r = subprocess.run(pre + list(a), capture_output=True, text=True, input=stdin, cwd=out, timeout=900)
        return r.returncode, r.stdout, r.stderr

    code, so, se = run(exe, "--version")
    if code != 0 or so.strip() != f"brevi {version}":
        fail(f"`brevi --version` printed {so.strip()!r} (exit {code}), expected 'brevi {version}'\n{se}")
    print(f"ok   {name}: brevi --version is {version}")

    bv = os.path.join(out, "smoke.bv")
    with open(bv, "w") as f:
        f.write('n = 1 + 1\nprint("hi {n}")\n')
    code, so, se = run(exe, "run", bv)
    if code != 0 or so.strip() != "hi 2":
        fail(f"typed smoke.bv printed {so!r} (exit {code})\n{se}")
    print(f"ok   {name}: builds and runs a typed .bv program")

    py = os.path.join(ROOT, "tests", "dyn", "classes.py")
    expected = open(os.path.join(ROOT, "tests", "dyn", "classes.expected")).read()
    env_dyn = dict(os.environ, BREVI_DYN="1", BREVI_PATH="1")
    r = subprocess.run(pre + [exe, "run", py, "--native"], capture_output=True, text=True, cwd=out, timeout=900, env=env_dyn)
    if r.returncode != 0 or r.stdout != expected:
        fail(f"tests/dyn/classes.py differs from its .expected (exit {r.returncode})\n{r.stdout}\n{r.stderr[-2000:]}")
    if '"path":"dynamic"' not in r.stderr:
        fail(f"tests/dyn/classes.py was not built on the dynamic path\n{r.stderr[-2000:]}")
    print(f"ok   {name}: builds and runs a Python program on the dynamic build")

    if not windows:
        code, so, se = run(os.path.join(d, "brevi-python"), "-c", "print(6 * 7)")
        if code != 0 or so.strip() != "42":
            fail(f"brevi-python -c printed {so!r} (exit {code})\n{se}")
        print(f"ok   {name}: brevi-python runs")
    print(exe)


if __name__ == "__main__":
    main()
