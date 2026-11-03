"""Brevi vs Codon vs Nuitka vs CPython on plain Python programs.

Every compiler gets the same unchanged .py files. For each program and tool it records whether the build
succeeds, how long a cold build takes, and for each test input whether stdout and the exit status match
what CPython prints (CPython is the reference, not the task's expected output), plus run time.

Corpora (--corpus):
  speed      speed/*.py with big generated inputs; best of --reps runs, release builds
  traps      traps/*.py: small programs that probe Python semantics compilers often get wrong
  solutions  every saved single-file and multi-file Python solution in ../solutions/*-python, run on its task's tests
  huge       the huge-set solutions (3-file chess / adventure / minilang programs) in $BREVI_HUGE

Tools (--tools, comma separated): cpython, brevi (brevi build --release), brevi-dev (brevi build, what
`brevi run` uses), codon (codon build -release -exe), nuitka (python -m nuitka --follow-imports).

usage: python3 compare.py --corpus traps --tools brevi,codon,nuitka [--jobs 4] [--out results/traps.jsonl]
       python3 compare.py --report results/*.jsonl      # markdown tables
Needs: target/release/brevi built; codon on PATH or $CODON; `pip install nuitka` and a C compiler (ccache helps).
"""
import argparse, concurrent.futures as cf, glob, json, os, shutil, subprocess, sys, tempfile, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
BENCH = HERE.parent
BREVI = os.environ.get("BREVI", str(BENCH.parent / "target" / "release" / "brevi"))
CODON = os.environ.get("CODON") or shutil.which("codon") or "codon"
HUGE = Path(os.environ.get("BREVI_HUGE", "bench/work/huge"))
PY = sys.executable
RUN_TIMEOUT = 60

# Input size per speed program, picked so CPython takes about 1 to 4 seconds.
SPEED = {"nbody": "500000", "fannkuch": "10", "spectral": "600", "trees": "16", "set_bfs": "700",
         "strings": "2000000", "objects": "3000000", "dp_lcs": "3000",
         "words": None, "graph": None, "sieve": None, "records": None}


def speed_inputs():
    d = HERE / "speed"
    if not (d / "records.in").exists():
        subprocess.run([PY, str(BENCH / "perf" / "gen.py")], cwd=d, check=True)
    for k, v in SPEED.items():
        if v is not None:
            (d / f"{k}.in").write_text(v + "\n")


def programs(corpus):
    """Yields (name, entry .py, folder to copy or None, [test .in files])."""
    if corpus == "speed":
        speed_inputs()
        for k in SPEED:
            yield f"speed/{k}", HERE / "speed" / f"{k}.py", None, [HERE / "speed" / f"{k}.in"]
    elif corpus == "traps":
        for p in sorted((HERE / "traps").glob("*.py")):
            yield f"traps/{p.stem}", p, None, [p.with_suffix(".in")]
    elif corpus == "solutions":
        seen = set()
        for p in sorted(glob.glob(str(BENCH / "solutions" / "*-python" / "*.py"))):
            p = Path(p)
            body = p.read_bytes()
            if body in seen:
                continue
            seen.add(body)
            tests = sorted((BENCH / "tasks" / p.stem / "tests").glob("*.in"))
            if tests:
                yield f"{p.parent.name}/{p.stem}", p, None, tests
        for d in sorted(glob.glob(str(BENCH / "solutions" / "*-python" / "*/"))):
            d = Path(d)
            tests = sorted((BENCH / "tasks" / d.name / "tests").glob("*.in"))
            if (d / "main.py").exists() and tests:
                yield f"{d.parent.name}/{d.name}", d / "main.py", d, tests
    elif corpus == "huge":
        for d in sorted(HUGE.glob("*/*/")):
            tests = sorted((BENCH / "tasks" / d.name / "tests").glob("*.in"))
            if (d / "main.py").exists() and tests:
                yield f"huge/{d.parent.name}/{d.name}", d / "main.py", d, tests


def build(tool, src, work):
    """Builds src (inside work) and returns (argv to run, build seconds, error or None)."""
    exe = work / "prog"
    env = dict(os.environ)
    if tool == "cpython":
        return [PY, str(src)], 0.0, None
    if tool in ("brevi", "brevi-dev"):
        env["BREVI_NO_CACHE"] = "1"
        cmd = [BREVI, "build", str(src), "-o", str(exe)] + (["--release"] if tool == "brevi" else [])
    elif tool == "codon":
        cmd = [CODON, "build", "-release", "-exe", str(src), "-o", str(exe)]
    elif tool == "nuitka":
        cmd = [PY, "-m", "nuitka", "--follow-imports", "--remove-output", "--quiet", "--assume-yes-for-downloads",
               f"--output-dir={work}", "--output-filename=prog", str(src)]
    else:
        raise SystemExit(f"unknown tool {tool}")
    t = time.perf_counter()
    try:
        r = subprocess.run(cmd, cwd=work, capture_output=True, text=True, timeout=900, env=env)
    except subprocess.TimeoutExpired:
        return None, 900.0, "build timeout"
    dt = time.perf_counter() - t
    if r.returncode != 0 or not exe.exists():
        lines = [l for l in (r.stdout + "\n" + r.stderr).splitlines() if l.strip() and "WARNING" not in l]
        err = next((l for l in lines if "error" in l.lower()), lines[0] if lines else f"exit {r.returncode}")
        return None, dt, strip_ansi(err)[:300]
    return [str(exe)], dt, None


def strip_ansi(s):
    import re
    return re.sub(r"\x1b\[[0-9;]*m", "", s)


def run(argv, inp, cwd, reps=1):
    best, out, rc = None, None, None
    for _ in range(reps):
        t = time.perf_counter()
        try:
            r = subprocess.run(argv, stdin=open(inp, "rb"), capture_output=True, cwd=cwd, timeout=RUN_TIMEOUT)
        except subprocess.TimeoutExpired:
            return "TIMEOUT", -999, RUN_TIMEOUT
        dt = time.perf_counter() - t
        best = dt if best is None else min(best, dt)
        out, rc = r.stdout, r.returncode
    return out, rc, best


def one(job):
    name, src, folder, tests, tool, reps = job
    work = Path(tempfile.mkdtemp(prefix="cmp-"))
    try:
        if folder:
            for f in folder.glob("*.py"):
                shutil.copy(f, work / f.name)
        else:
            shutil.copy(src, work / src.name)
        entry = work / src.name
        argv, bt, err = build(tool, entry, work)
        row = {"prog": name, "tool": tool, "build_ok": err is None, "build_s": round(bt, 2), "err": err, "tests": []}
        if err is None:
            for i in tests:
                ref = REF[(name, str(i))]
                out, rc, dt = run(argv, i, work, reps)
                row["tests"].append({"in": i.name, "same_out": out == ref[0], "same_rc": (rc == 0) == (ref[1] == 0),
                                     "rc": rc, "s": round(dt, 4), "timeout": out == "TIMEOUT"})
        return row
    finally:
        shutil.rmtree(work, ignore_errors=True)


REF = {}


def reference(progs, reps):
    """CPython's stdout and exit status for every test, and its time."""
    rows = []
    for name, src, folder, tests in progs:
        work = Path(tempfile.mkdtemp(prefix="ref-"))
        try:
            if folder:
                for f in folder.glob("*.py"):
                    shutil.copy(f, work / f.name)
            else:
                shutil.copy(src, work / src.name)
            row = {"prog": name, "tool": "cpython", "build_ok": True, "build_s": 0.0, "err": None, "tests": []}
            for i in tests:
                out, rc, dt = run([PY, str(work / src.name)], i, work, reps)
                REF[(name, str(i))] = (out, rc)
                row["tests"].append({"in": i.name, "same_out": True, "same_rc": True, "rc": rc, "s": round(dt, 4), "timeout": out == "TIMEOUT"})
            rows.append(row)
        finally:
            shutil.rmtree(work, ignore_errors=True)
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default="traps")
    ap.add_argument("--tools", default="brevi,codon,nuitka")
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    ap.add_argument("--reps", type=int, default=0, help="runs per test, best kept (default 3 for speed, else 1)")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--out")
    ap.add_argument("--report", nargs="*")
    a = ap.parse_args()
    if a.report is not None:
        return report([Path(p) for p in a.report])
    reps = a.reps or (3 if a.corpus == "speed" else 1)
    progs = list(programs(a.corpus))
    if a.limit:
        progs = progs[: a.limit]
    out = Path(a.out or HERE / "results" / f"{a.corpus}.jsonl")
    out.parent.mkdir(exist_ok=True)
    rows = reference(progs, reps)
    with open(out, "w") as f:
        for r in rows:
            f.write(json.dumps(r) + "\n")
    jobs = [(n, s, d, t, tool, reps) for n, s, d, t in progs for tool in a.tools.split(",") if tool != "cpython"]
    # Speed runs one at a time so timings do not compete for cores.
    workers = 1 if a.corpus == "speed" else a.jobs
    done = 0
    with cf.ThreadPoolExecutor(workers) as ex, open(out, "a") as f:
        for r in ex.map(one, jobs):
            f.write(json.dumps(r) + "\n")
            f.flush()
            done += 1
            ok = r["build_ok"] and all(t["same_out"] and t["same_rc"] for t in r["tests"])
            print(f"[{done}/{len(jobs)}] {r['tool']:9} {r['prog']:40} {'OK' if ok else ('BUILD FAIL' if not r['build_ok'] else 'DIFF')} {r['build_s']}s", flush=True)


def report(paths):
    rows = [json.loads(l) for p in paths for l in open(p)]
    tools = [t for t in ["cpython", "brevi", "brevi-dev", "codon", "nuitka"] if any(r["tool"] == t for r in rows)]
    by = {}
    for r in rows:
        by.setdefault(r["prog"], {})[r["tool"]] = r
    print("| Tool | Builds unchanged | Matches CPython on every test | Wrong output (built but differs) | Median build s |")
    print("| --- | --- | --- | --- | --- |")
    n = len(by)
    for t in tools:
        rs = [p[t] for p in by.values() if t in p]
        built = [r for r in rs if r["build_ok"]]
        right = [r for r in built if all(x["same_out"] and x["same_rc"] for x in r["tests"])]
        bs = sorted(r["build_s"] for r in rs)
        med = bs[len(bs) // 2] if bs else 0
        print(f"| {t} | {len(built)}/{n} | {len(right)}/{n} | {len(built) - len(right)} | {med:.1f} |")
    if any(p.startswith("speed/") for p in by):
        print()
        print("| Program | " + " | ".join(tools) + " |")
        print("| --- |" + " --- |" * len(tools))
        for p, d in by.items():
            base = d["cpython"]["tests"][0]["s"]
            cells = []
            for t in tools:
                r = d.get(t)
                if r is None:
                    cells.append("")
                elif not r["build_ok"]:
                    cells.append("no build")
                else:
                    x = r["tests"][0]
                    tag = "" if x["same_out"] and x["same_rc"] else " (wrong)"
                    cells.append(f"{x['s']:.2f}s" + ("" if t == "cpython" else f" ({base / x['s']:.1f}x)") + tag)
            print(f"| {p[6:]} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    main()
