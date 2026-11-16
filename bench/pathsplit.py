"""Path-split case study: on agent-written Python, what share takes the typed, dynamic and pybridge builds, and how
fast each runs against CPython (docs/CASE_STUDY_PATHS.md).

The workload is every saved Python solution a model wrote in the e2e benchmark (bench/solutions/*-python: one file
per task, or a folder with main.py), run on its task's tests (bench/tasks/<task>/tests). Each one is built with
BREVI_PATH=1 (the build prints which path it took), then CPython and the native binary run every test, one at a
time, best of REPS per test; a program's time is the sum over its tests. Speed is counted only where the native
binary printed exactly what CPython printed on every test; any difference is a wrong answer and is listed.

Model-written code never runs outside tools/modelbox.py, so the outer call boxes the inner one:

usage: BREVI=path/to/brevi python3 bench/pathsplit.py OUT.json      (builds and runs everything in the box)
       python3 bench/pathsplit.py --report OUT.json                 (prints the tables the docs page quotes)
"""
import collections, glob, hashlib, re, json, os, shutil, statistics, subprocess, sys, tempfile, time, concurrent.futures as cf
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
REPS = int(os.environ.get("PATHSPLIT_REPS", "5"))
JOBS = int(os.environ.get("PATHSPLIT_JOBS", "3"))


def programs():
    """(task, source, folder?) for each distinct saved Python solution that has tests."""
    seen, out = set(), []
    for d in sorted(glob.glob(str(REPO / "bench/solutions/*-python"))):
        for p in sorted(Path(d).iterdir()):
            if p.is_file() and p.suffix == ".py":
                task, files = p.stem, [p]
            elif p.is_dir() and (p / "main.py").is_file():
                task, files = p.name, sorted(p.glob("*.py"))
            else:
                continue
            if not (REPO / "bench/tasks" / task / "tests").is_dir():
                continue
            h = hashlib.sha256(b"".join(f.name.encode() + b"\0" + f.read_bytes() for f in files)).hexdigest()
            if h in seen:
                continue  # the same program saved twice counts once
            seen.add(h)
            out.append({"task": task, "src": str(p.relative_to(REPO)), "folder": p.is_dir()})
    return out


def best(cmd, data, cwd, env):
    """(min wall ms over REPS, stdout, failed?, stderr) for one run of cmd on data."""
    ms, outs = [], None
    for _ in range(REPS):
        t0 = time.perf_counter()
        try:
            r = subprocess.run(cmd, input=data, capture_output=True, timeout=60, cwd=cwd, env=env)
        except subprocess.TimeoutExpired:
            return None, b"", True, b""
        ms.append((time.perf_counter() - t0) * 1000)
        outs = (r.stdout, r.returncode != 0, r.stderr)
    return min(ms), outs[0], outs[1], outs[2]


def build(p, work):
    t = Path(tempfile.mkdtemp(dir=work))
    src = REPO / p["src"]
    if p["folder"]:
        for f in src.glob("*.py"):
            shutil.copy(f, t)
        main = t / "main.py"
    else:
        main = t / src.name
        shutil.copy(src, main)
    env = dict(os.environ, BREVI_PATH="1")
    t0 = time.perf_counter()
    try:
        r = subprocess.run([os.environ["BREVI"], "build", str(main), "-o", str(t / "bin")], capture_output=True,
                           text=True, timeout=600, env=env, cwd=t)
    except subprocess.TimeoutExpired:
        return dict(p, path="timeout", dir=str(t), main=str(main))
    rec = None
    for line in r.stderr.splitlines():
        if line.startswith('{"path"'):
            try:
                rec = json.loads(line)
            except ValueError:
                pass
    if r.returncode != 0 or not (t / "bin").exists():
        why = (rec or {}).get("why") or (r.stderr.strip().splitlines() or [""])[-1][:300]
        return dict(p, path="refused", why=why, dir=str(t), main=str(main))
    return dict(p, path=(rec or {}).get("path", "unknown"), why=(rec or {}).get("why", ""), build_s=round(time.perf_counter() - t0, 1), dir=str(t),
                main=str(main))


def measure(rec):
    tests = sorted((REPO / "bench/tasks" / rec["task"] / "tests").glob("*.in"))
    env_py = dict(os.environ, PYTHONHASHSEED="0")
    cpy_ms = nat_ms = 0.0
    cpy_ok, differs, stopped = True, [], []
    for i in tests:
        data = i.read_bytes()
        want = i.with_suffix(".out").read_bytes() if i.with_suffix(".out").exists() else None
        pm, pout, pfail, _ = best([sys.executable, rec["main"]], data, rec["dir"], env_py)
        if pm is None or (want is not None and pout != want):
            cpy_ok = False
        if rec["path"] in ("refused", "timeout"):
            cpy_ms += pm or 0
            continue
        nm, nout, nfail, nerr = best([os.path.join(rec["dir"], "bin")], data, rec["dir"], env_py)
        if nm is None or (nout, nfail) != (pout, pfail):
            differs.append(i.name)
            # the typed build's checked stop (an int past 64 bits, ...): an error and exit 1, never a wrong answer
            if nfail and b"the typed build stopped" in nerr and pout.startswith(nout):
                stopped.append(i.name)
        cpy_ms += pm or 0
        nat_ms += nm or 0
    rec.update(tests=len(tests), cpython_ok=cpy_ok, cpython_ms=round(cpy_ms, 2))
    if rec["path"] not in ("refused", "timeout"):
        rec.update(native_ms=round(nat_ms, 2), differs=differs, stopped=stopped, match=not differs,
                   wrong=sorted(set(differs) - set(stopped)))
    return rec


def startup(work):
    """Floor costs: CPython starting up and exiting, and a native binary of each path doing the same."""
    t = Path(tempfile.mkdtemp(dir=work))
    (t / "typed.py").write_text("print(1)\n")
    out = {}
    out["cpython"] = best([sys.executable, str(t / "typed.py")], b"", t, None)[0]
    for name, extra in (("typed", {}), ("dyn", {"BREVI_DYN": "1"})):
        r = subprocess.run([os.environ["BREVI"], "build", str(t / "typed.py"), "-o", str(t / name)],
                           capture_output=True, text=True, env=dict(os.environ, BREVI_PATH="1", **extra), cwd=t)
        path = next((json.loads(l)["path"] for l in r.stderr.splitlines() if l.startswith('{"path"')), "?")
        out[f"native_{path}"] = best([str(t / name)], b"", t, None)[0]
    return {k: round(v, 2) for k, v in out.items()}


def inner(out):
    work = Path(out).resolve().parent / "pathsplit-work"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    ps = programs()
    with cf.ThreadPoolExecutor(JOBS) as ex:  # builds in parallel; timing below runs one at a time
        recs = list(ex.map(lambda p: build(p, work), ps))
    res = {"brevi": subprocess.run([os.environ["BREVI"], "--version"], capture_output=True, text=True).stdout.strip(),
           "python": sys.version.split()[0], "reps": REPS, "startup_ms": startup(work), "programs": []}
    for r in recs:
        res["programs"].append(measure(r))
    for r in res["programs"]:
        r.pop("dir", None), r.pop("main", None)
    Path(out).write_text(json.dumps(res, indent=1))
    shutil.rmtree(work, ignore_errors=True)


def pct(a, b):
    return f"{a} of {b} ({100 * a / b:.1f}%)" if b else f"{a} of 0"


def report(path):
    res = json.load(open(path))
    ps = [p for p in res["programs"] if p["cpython_ok"]]
    fl = res["startup_ms"]
    print(f"Brevi {res['brevi']}, CPython {res['python']}, best of {res['reps']}")
    print(f"distinct programs: {len(res['programs'])}; CPython passes its tests: {len(ps)} (the workload)")
    print("start-up floors, ms (a program that prints one line):", fl)
    for path_ in ("typed", "dynamic", "pybridge", "refused", "timeout"):
        sub = [p for p in ps if p["path"] == path_]
        print(f"\n{path_}: {pct(len(sub), len(ps))}")
        if path_ in ("refused", "timeout") or not sub:
            for p in sub[:40]:
                print("   ", p["src"], (p.get("why") or "")[:140])
            continue
        ok = [p for p in sub if p["match"]]
        bad = [p for p in sub if p["wrong"]]
        stop = [p for p in sub if p["differs"] and not p["wrong"]]
        print(f"  match CPython on every test: {len(ok)}; stopped with the typed build's checked error: {len(stop)};"
              f" wrong answers: {len(bad)}")
        for p in stop:
            print("    STOPPED", p["src"], p["stopped"][:5])
        for p in bad:
            print("    WRONG", p["src"], p["wrong"][:5])
        sp = sorted(p["cpython_ms"] / p["native_ms"] for p in ok if p["native_ms"] > 0)
        print(f"  whole-run speed-up over CPython: median {times(statistics.median(sp))}, range {times(sp[0])} to"
              f" {times(sp[-1])}, slower than CPython {sum(1 for x in sp if x < 1)} of {len(sp)}")
        cpt = statistics.median(p["cpython_ms"] / p["tests"] for p in ok)
        nat = statistics.median(p["native_ms"] / p["tests"] for p in ok)
        print(f"  median time per test: CPython {cpt:.1f} ms, native {nat:.1f} ms")
    print("\ntyped share by program size (lines of Python, all files):")
    for lo, hi in ((0, 40), (40, 80), (80, 150), (150, None)):
        sub = [p for p in ps if lo <= lines(p) and (hi is None or lines(p) < hi)]
        print(f"  {lo}-{hi or ''} lines: typed {pct(sum(p['path'] == 'typed' for p in sub), len(sub))}")
    print(f"  median size: {statistics.median(lines(p) for p in ps)} lines")
    kinds = collections.Counter(kind(p["why"]) for p in ps if p["path"] == "dynamic")
    print("\nwhy the dynamic programs left the typed build, grouped:")
    for w, n in kinds.most_common():
        print(f"  {n:4}  {w}")
    why = collections.Counter(re.sub(r"`[^`]*`", "`...`", p["why"]) for p in ps if p["path"] == "dynamic")
    print("\nwhy the dynamic programs left the typed build (first reason the build gave):")
    for w, n in why.most_common(12):
        print(f"  {n:4}  {w[:150]}")


KINDS = (("set or dict order: CPython's hash order", ("hash order",)),
         ("`is` needs object identity", ("`is` between",)),
         ("two names share one list and one changes", ("name the same list", "a copy of", "shares a list")),
         ("the typed build's generated Rust did not compile", ("Rust did not compile",)),
         ("a library or method not in the typed build", ("not in the typed build", "unknown method", "re.findall",
                                                         "keeps as a plain string")),
         ("a value changes type", ("but is set to", "holds a different type", "takes ", "but gets")),
         ("an int past 64 bits", ("past 64 bits",)))


def kind(why):
    for name, keys in KINDS:
        if any(k in why for k in keys):
            return name
    return "other"


def lines(p):
    s = REPO / p["src"]
    return sum(len(f.read_text().splitlines()) for f in (sorted(s.glob("*.py")) if p["folder"] else [s]))


def times(x):
    return f"{x:.2f}x" if x < 1 else f"{x:.1f}x"


if __name__ == "__main__":
    if sys.argv[1:2] == ["--report"]:
        report(sys.argv[2])
    elif sys.argv[1:2] == ["--inner"]:
        os.environ["BREVI"] = sys.argv[3]  # the box passes only BREVI_* variables
        inner(sys.argv[2])
    else:
        out = Path(sys.argv[1]).resolve()
        if REPO / "target" not in out.parents:
            sys.exit("OUT must be under target/ (the only folder the box can write)")
        if not os.environ.get("BREVI"):
            sys.exit("set BREVI to the brevi binary (under target/, so the box can see it)")
        sys.exit(subprocess.call([sys.executable, str(REPO / "tools/modelbox.py"), sys.executable, "-I",
                                  str(REPO / "bench/pathsplit.py"), "--inner", str(out),
                                  str(Path(os.environ["BREVI"]).resolve())], cwd=REPO))
