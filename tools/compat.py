#!/usr/bin/env python3
"""Compatibility check: code that once passed must keep passing.

Builds every saved benchmark solution (Brevi, Python and Rust) with the toolchain on PATH,
runs it on the task's tests and compares the output with the expected .out files.
compat/baseline.txt lists every solution that passed when the baseline was written;
any of those that now fails is a regression and makes this script exit 1.

usage: compat.py check  [--kinds brevi,python,rust] [--report FILE]
       compat.py baseline [--reset]  (add newly passing solutions to compat/baseline.txt; never drops one without --reset)
       compat.py lints  [--report FILE]
                                    (compile the Rust that Brevi generates with deprecation and
                                     future-incompatibility warnings on, and with the newest edition;
                                     prints what a future rustc will start rejecting)
Env: BREVI (default target/release/brevi), JOBS (default: CPU count).
"""
import concurrent.futures as cf, json, os, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCH = ROOT / "bench"
BASELINE = ROOT / "compat" / "baseline.txt"
BREVI = os.environ.get("BREVI", str(ROOT / "target" / "release" / "brevi"))
EXT = {".bv": "brevi", ".ax": "brevi", ".py": "python", ".rs": "rust"}


def solutions(kinds):
    for f in sorted(BENCH.glob("solutions/*/*")):
        k = EXT.get(f.suffix)
        if k in kinds and (BENCH / "tasks" / f.stem / "tests").is_dir():
            yield f, k


def run(cmd, **kw):
    try:
        return subprocess.run(cmd, capture_output=True, text=True, timeout=kw.pop("timeout", 120), **kw)
    except subprocess.TimeoutExpired:
        return None


def one(f, kind):
    """Returns (relative path, None if it passes else a short reason)."""
    rel = str(f.relative_to(ROOT))
    with tempfile.TemporaryDirectory() as t:
        exe = Path(t) / "prog"
        if kind == "brevi":
            r = run([BREVI, "build", str(f), "-o", str(exe)])
        elif kind == "rust":
            r = run(["rustc", "--edition", "2021", "-A", "warnings", "-o", str(exe), str(f)])
        else:
            r = run([sys.executable, "-m", "py_compile", str(f)])
        if r is None or r.returncode != 0:
            return rel, "build failed: " + ((r.stdout + r.stderr).strip()[:400] if r else "timeout")
        cmd = [sys.executable, str(f)] if kind == "python" else [str(exe)]
        for i in sorted((BENCH / "tasks" / f.stem / "tests").glob("*.in")):
            r = run(cmd, stdin=i.open(), timeout=10)
            got = "" if r is None else (r.stdout + r.stderr)
            if got.rstrip("\n") != i.with_suffix(".out").read_text().rstrip("\n"):
                return rel, f"wrong output on {i.name}" + (" (timeout)" if r is None else "")
    return rel, None


def check_all(kinds):
    jobs = int(os.environ.get("JOBS", os.cpu_count() or 4))
    with cf.ThreadPoolExecutor(jobs) as ex:
        return dict(ex.map(lambda a: one(*a), solutions(kinds)))


def arg(name, default=None):
    a = sys.argv
    return a[a.index(name) + 1] if name in a and a.index(name) + 1 < len(a) else default


def versions():
    v = {"python": sys.version.split()[0]}
    for k, cmd in [("rustc", ["rustc", "-V"]), ("brevi", [BREVI, "--version"])]:
        r = run(cmd)
        v[k] = r.stdout.strip() if r and r.returncode == 0 else "?"
    return v


def cmd_check():
    kinds = set(arg("--kinds", "brevi,python,rust").split(","))
    base = [l for l in BASELINE.read_text().split() if EXT.get(Path(l).suffix) in kinds]
    res = check_all(kinds)
    broken = {p: res.get(p, "solution file missing") for p in base if res.get(p, "missing") is not None}
    newly = sorted(p for p, why in res.items() if why is None and p not in base)
    v = versions()
    lines = [f"Toolchain: {v['rustc']}, Python {v['python']}, {v['brevi']}",
             f"Baseline solutions checked: {len(base)} ({', '.join(sorted(kinds))}); regressions: {len(broken)}"]
    if broken:
        lines += ["", "Solutions that passed before and now fail:"] + [f"- `{p}`: {why}" for p, why in sorted(broken.items())]
    if newly:
        lines += ["", f"{len(newly)} solutions pass now that were not in the baseline (run `tools/compat.py baseline` to add them)."]
    text = "\n".join(lines)
    print(text)
    if arg("--report"):
        Path(arg("--report")).write_text(text + "\n")
    return 1 if broken else 0


def cmd_baseline():
    """Adds newly passing solutions. Never drops a line unless --reset is given."""
    res = check_all({"brevi", "python", "rust"})
    old = set(BASELINE.read_text().split()) if BASELINE.exists() else set()
    ok = {p for p, why in res.items() if why is None}
    failing = sorted(p for p in old if p not in ok)
    keep = ok if "--reset" in sys.argv else ok | old
    BASELINE.parent.mkdir(exist_ok=True)
    BASELINE.write_text("\n".join(sorted(keep)) + "\n")
    print(f"{len(ok)} of {len(res)} solutions pass; baseline now has {len(keep)} (added {len(ok - old)})")
    if failing:
        print(f"{len(failing)} baseline solutions fail now and were " + ("dropped (--reset)" if "--reset" in sys.argv else "kept") + ":")
        print("\n".join("  " + p for p in failing))
    return 1 if failing and "--reset" not in sys.argv else 0


def lint_one(f):
    """Warnings (not errors: build falls back to a plain version when a move fails) rustc gives on the Rust that Brevi generates for f, for the current and newest edition."""
    r = run([BREVI, "emit", str(f)])
    if r is None or r.returncode != 0:
        return []
    found = []
    with tempfile.TemporaryDirectory() as t:
        rs = Path(t) / "main.rs"
        rs.write_text(r.stdout)
        for ed in ["2021", NEWEST_EDITION]:
            c = run(["rustc", "--edition", ed, "--error-format=json", "--emit=metadata", "--out-dir", t,
                     "--crate-name", "brevi_prog", "-A", "warnings", "-W", "deprecated", "-W", "future-incompatible",
                     "-W", "rust-2024-compatibility", str(rs)])
            if c is None:
                continue
            for l in c.stderr.splitlines():
                try:
                    j = json.loads(l)
                except ValueError:
                    continue
                if j.get("level") == "warning" and not j["message"].startswith(("aborting", "unused", "1 warning", "warning")):
                    code = (j.get("code") or {}).get("code", "")
                    found.append((ed, j["level"], code, j["message"].split("\n")[0]))
    return found


def newest_edition():
    best = "2021"
    for ed in ["2024", "2027", "2030"]:
        r = run(["rustc", "--edition", ed, "--print", "sysroot"])
        if r and r.returncode == 0:
            best = ed
    return best


def cmd_lints():
    global NEWEST_EDITION
    NEWEST_EDITION = newest_edition()
    base = [ROOT / l for l in BASELINE.read_text().split() if EXT.get(Path(l).suffix) == "brevi"]
    seen = {}
    jobs = int(os.environ.get("JOBS", os.cpu_count() or 4))
    with cf.ThreadPoolExecutor(jobs) as ex:
        for f, found in zip(base, ex.map(lint_one, base)):
            for w in found:
                seen.setdefault(w, []).append(str(f.relative_to(ROOT)))
    v = versions()
    lines = [f"Toolchain: {v['rustc']}; editions checked: 2021 (used now) and {NEWEST_EDITION} (newest this rustc knows)",
             f"Generated programs checked: {len(base)}; distinct warnings: {len(seen)}"]
    for (ed, lvl, code, msg), files in sorted(seen.items(), key=lambda kv: -len(kv[1])):
        lines.append(f"- edition {ed} {lvl} {code}: {msg} ({len(files)} programs, e.g. `{files[0]}`)")
    text = "\n".join(lines)
    print(text)
    if arg("--report"):
        Path(arg("--report")).write_text(text + "\n")
    return 0


NEWEST_EDITION = "2021"
if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    sys.exit({"check": cmd_check, "baseline": cmd_baseline, "lints": cmd_lints}.get(cmd, lambda: print(__doc__) or 2)())
