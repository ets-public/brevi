# Times the showcase programs built with Brevi against CPython running the same files.
#
#   python3 bench/showcase/run.py [--runs N] [--only NAME]
#
# Each showcase is built once with `brevi build --release`, then both sides run it N times (default 10) and the
# median is kept. Output must match between the two sides, or the showcase is reported as a mismatch.
# Results go to bench/showcase/results.json; the table on site/docs/showcases.md is written from it. Each result
# carries a size breakdown of its binary (size.py: sections, code and data by crate, largest symbols).
#
#   python3 bench/showcase/run.py --gate [--targets]
#
# The size and startup gate (bench/showcase/SIZE.md): builds and times the GATED showcases and fails (exit 1) when a
# binary is more than 5% bigger, its first answer more than 10% slower, or its release build more than 30% slower or
# 30% hungrier for memory than bench/showcase/baseline.json, or when its output differs from CPython's. Startup is only comparable on the machine the baseline was taken on, so a
# baseline from another CPU is refused (exit 2) rather than skipped. --targets also fails on a binary over its size
# target (TARGETS). The gate writes no results.json; `--record-baseline` writes baseline.json from this run.
import importlib.util
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

import size as size_tool

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
BREVI = os.environ.get("BREVI", os.path.join(ROOT, "target", "release", "brevi"))


def mcp_session(cmd, script, cwd, env=None):
    # Lockstep, as a client does: each request goes once the reply before it is in. Returns the replies,
    # the time to the first tools/call reply and the time for the whole session.
    t0 = time.perf_counter()
    p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=cwd, env=env)
    replies, first = [], None
    for line in open(script).read().splitlines():
        msg = json.loads(line)
        p.stdin.write((line + "\n").encode())
        p.stdin.flush()
        if msg.get("id") is None:
            continue
        reply = p.stdout.readline().decode()
        replies.append(reply)
        if first is None and msg.get("method") == "tools/call":
            first = time.perf_counter() - t0
    p.stdin.close()
    p.wait(timeout=30)
    return replies, first, time.perf_counter() - t0


def cli_run(cmd, cwd, env=None):
    t0 = time.perf_counter()
    r = subprocess.run(cmd, capture_output=True, cwd=cwd, timeout=600, env=env)
    t = time.perf_counter() - t0
    return r.stdout.decode() + "exit %d" % r.returncode, t, t


def timed_build(cmd):
    # Wall time and peak memory of a build. ru_maxrss from wait4 is the largest resident size of the build or any of
    # the processes it waited for (rustc, the linker): the memory a machine needs for the build, in MB (Linux: KiB).
    t0 = time.perf_counter()
    p = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    _, status, ru = os.wait4(p.pid, 0)
    if os.waitstatus_to_exitcode(status) != 0:
        return None, None
    return time.perf_counter() - t0, round(ru.ru_maxrss / 1024, 1)


def links_python(binary):
    try:
        out = subprocess.run(["ldd", binary], capture_output=True, text=True).stdout
    except OSError:
        return None
    return "libpython" in out


# The bundler showcases (an MCP server or a CLI script turned into one binary) that the gate holds. micrograd and
# microgpt are throughput demos that take minutes; they are measured, not gated.
GATED = ["sqlite_mcp", "time_mcp", "count_tokens_file", "count_tokens_tree"]
SIZE_GROWTH = 1.05  # a binary may grow at most 5% over the baseline
START_GROWTH = 1.10  # and its first answer may be at most 10% slower
BUILD_GROWTH = 1.30  # its release build may take at most 30% longer (Option A plan, lane A2)
PEAK_GROWTH = 1.30  # and need at most 30% more memory at its peak
# Size targets (Option A, Adam 11-19): time_mcp at most 8 MB, every showcase at most 10 MB. Bytes, as MB = 1e6.
TARGETS = {"time_mcp": 8_000_000}
TARGET_DEFAULT = 10_000_000

SHOWCASES = [
    {
        "name": "sqlite_mcp",
        "title": "SQLite MCP server",
        "file": "sqlite_mcp.py",
        "kind": "mcp",
        "script": "sqlite_mcp.jsonl",
        "metric": "first tool call answered",
    },
    {
        "name": "time_mcp",
        "title": "MCP time server",
        "file": "time_mcp.py",
        "kind": "mcp",
        "script": "time_mcp.jsonl",
        "metric": "first tool call answered",
        # the upstream file, unchanged, from the installed package
        "beside": {"server.py": ("mcp_server_time", "server.py")},
        "env": {"TZ": "Europe/Paris"},
    },
    {
        "name": "count_tokens_file",
        "title": "Token count, one file",
        "file": "count_tokens.py",
        "kind": "cli",
        "args": [os.path.join(ROOT, "README.md")],
        "metric": "count of one file printed",
    },
    {
        "name": "count_tokens_tree",
        "title": "Token count, a source tree",
        "file": "count_tokens.py",
        "kind": "cli",
        "args": [os.path.join(ROOT, "src"), os.path.join(ROOT, "site", "docs"), "--ext", ".rs", ".py", ".md"],
        "metric": "Brevi's own source counted",
    },
    {
        "name": "micrograd",
        "title": "micrograd, an MLP trained for 30 steps",
        "file": "micrograd_train.py",
        "kind": "cli",
        # Karpathy's package, unchanged, from the installed copy (pip install micrograd)
        "beside": {"micrograd": ("micrograd", None)},
        "metric": "training finished",
    },
    {
        "name": "microgpt",
        "title": "microgpt, a GPT trained for 1000 steps and sampled",
        "file": "microgpt.py",
        "kind": "cli",
        # Karpathy's gist, unchanged, and the names it trains on, fetched once into bench/work/fetched
        "fetch": {
            "microgpt.py": ("https://gist.githubusercontent.com/karpathy/8627fe009c40f57531cb18360106ce95/raw/microgpt.py",
                            "d47d88c2fd432c8ebdc1048beab7f7eb64ea7e0e664e11b812d72a6d95ebccee"),
            "input.txt": ("https://raw.githubusercontent.com/karpathy/makemore/988aa59/names.txt",
                          "0a30b5557f192f32ab962680889aac5f6fda0f4cecf40a6d0b5694f58ea8cc4d"),
        },
        "data": ["input.txt"],  # copied into each run's folder, so the program reads it instead of downloading it
        "runs": 3,  # about 4 minutes a run for both sides together
        "metric": "training and sampling finished",
    },
]


def fetch(files):
    # Downloads each file once and checks it is the version the numbers were measured with.
    import hashlib
    import urllib.request
    d = os.path.join(ROOT, "bench", "work", "fetched")
    os.makedirs(d, exist_ok=True)
    for name, (url, sha) in files.items():
        p = os.path.join(d, name)
        if not os.path.exists(p):
            urllib.request.urlretrieve(url, p)
        got = hashlib.sha256(open(p, "rb").read()).hexdigest()
        if got != sha:
            return d, "%s is not the version measured (sha256 %s, want %s): delete it to fetch again" % (p, got, sha)
    return d, None


def measure(sc, runs, work):
    src = os.path.join(HERE, sc["file"])
    data = []
    if sc.get("fetch"):
        fd, err = fetch(sc["fetch"])
        if err:
            return {"name": sc["name"], "error": err}
        src = os.path.join(fd, sc["file"])
        data = [os.path.join(fd, f) for f in sc.get("data", [])]
    if sc.get("beside"):
        d = os.path.join(work, sc["name"] + "_src")
        os.makedirs(d)
        shutil.copy(src, d)
        for name, (pkg, f) in sc["beside"].items():
            spec = importlib.util.find_spec(pkg)
            if spec is None:
                return {"name": sc["name"], "error": "%s is not installed (pip install %s)" % (pkg, pkg.replace("_", "-"))}
            if f is None:  # the whole package, unchanged
                shutil.copytree(os.path.dirname(spec.origin), os.path.join(d, name), ignore=shutil.ignore_patterns("__pycache__"))
            else:
                shutil.copy(os.path.join(os.path.dirname(spec.origin), f), os.path.join(d, name))
        src = os.path.join(d, sc["file"])
    exe = os.path.join(work, sc["name"])
    b = subprocess.run([BREVI, "build", src, "--release", "-o", exe], capture_output=True, text=True)
    if b.returncode != 0:
        return {"name": sc["name"], "error": (b.stdout + b.stderr).strip()[-400:]}
    # The build cost, timed on a second build: the first one above leaves the runtime rlib built and cached, and
    # --no-cache makes this one run rustc on the program again, as a rebuild after an edit does.
    build_s, build_peak = timed_build([BREVI, "build", src, "--release", "--no-cache", "-o", exe + "_rebuilt"])
    if build_s is None:
        return {"name": sc["name"], "error": "the second build, with --no-cache, failed"}
    sides = {"brevi": [exe], "cpython": [sys.executable, src]}
    env = dict(os.environ, **sc.get("env", {}))
    res = {}
    outs = {}
    for side, cmd in sides.items():
        firsts, totals = [], []
        for i in range(sc.get("runs", runs)):
            d = tempfile.mkdtemp(dir=work)
            for f in data:
                shutil.copy(f, d)
            if sc["kind"] == "mcp":
                out, first, total = mcp_session(cmd + sc.get("args", []), os.path.join(HERE, sc["script"]), d, env)
            else:
                out, first, total = cli_run(cmd + sc.get("args", []), d, env)
            shutil.rmtree(d, ignore_errors=True)
            outs.setdefault(side, out)
            firsts.append(first)
            totals.append(total)
        res[side] = {"first_s": statistics.median(firsts), "total_s": statistics.median(totals)}
    return {
        "name": sc["name"], "title": sc["title"], "metric": sc["metric"], "runs": sc.get("runs", runs),
        "same_output": outs["brevi"] == outs["cpython"],
        "brevi": res["brevi"], "cpython": res["cpython"],
        "speedup": res["cpython"]["first_s"] / res["brevi"]["first_s"],
        "binary_mb": round(os.path.getsize(exe) / 1e6, 1), "binary_bytes": os.path.getsize(exe),
        "needs_python": links_python(exe), "build_s": build_s,
        "build_peak_mb": build_peak, "size": size_tool.breakdown(exe),
    }


def cpu_name():
    try:
        for line in open("/proc/cpuinfo"):
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or None


def machine():
    return "%s %s, %d cores" % (platform.system(), platform.machine(), os.cpu_count() or 0)


def gate(results, base, targets, remeasure):
    # Returns the failures, one line each. A startup regression is measured again once before it counts, as one slow
    # run of the machine should not fail a PR; a size change is exact and is not.
    fails = []
    for name in GATED:
        r = results.get(name)
        if r is None or "error" in r:
            fails.append("%s: not built: %s" % (name, (r or {}).get("error", "not run")))
            continue
        if not r["same_output"]:
            fails.append("%s: output differs from CPython" % name)
        target = TARGETS.get(name, TARGET_DEFAULT)
        over = r["binary_bytes"] > target
        print("%-18s %6.2f MB (target %.0f MB%s)  first answer %6.1f ms  build %5.1f s, peak %6.0f MB" % (
            name, r["binary_bytes"] / 1e6, target / 1e6, ", OVER" if over else "", r["brevi"]["first_s"] * 1e3,
            r["build_s"], r["build_peak_mb"]))
        if targets and over:
            fails.append("%s: %.2f MB is over its %.0f MB target" % (name, r["binary_bytes"] / 1e6, target / 1e6))
        b = base["showcases"].get(name)
        if b is None:
            fails.append("%s: not in baseline.json (record it with --record-baseline)" % name)
            continue
        if r["binary_bytes"] > b["binary_bytes"] * SIZE_GROWTH:
            fails.append("%s: binary %.2f MB, baseline %.2f MB (+%.1f%%, limit +%.0f%%)" % (
                name, r["binary_bytes"] / 1e6, b["binary_bytes"] / 1e6,
                (r["binary_bytes"] / b["binary_bytes"] - 1) * 100, (SIZE_GROWTH - 1) * 100))
        # Timed numbers vary run to run, so a regression is measured a second time and the better result counts.
        checks = [("first answer", lambda x: x["brevi"]["first_s"], b.get("first_s"), START_GROWTH, 1e3, "ms"),
                  ("release build", lambda x: x["build_s"], b.get("build_s"), BUILD_GROWTH, 1, "s"),
                  ("build peak memory", lambda x: x["build_peak_mb"], b.get("build_peak_mb"), PEAK_GROWTH, 1, "MB")]
        again = None
        for what, get, was, limit, scale, unit in checks:
            if was is None:
                fails.append("%s: baseline.json has no %s (record it again with --record-baseline)" % (name, what))
                continue
            now = get(r)
            if now > was * limit:
                if again is None:
                    again = remeasure(name) or {"error": "not measured"}
                if "error" not in again:
                    now = min(now, get(again))
            if now > was * limit:
                fails.append("%s: %s %.1f %s, baseline %.1f %s (+%.0f%%, limit +%.0f%%; measured twice)" % (
                    name, what, now * scale, unit, was * scale, unit, (now / was - 1) * 100, (limit - 1) * 100))
    return fails


def main():
    args = sys.argv[1:]
    runs = int(args[args.index("--runs") + 1]) if "--runs" in args else 10
    only = args[args.index("--only") + 1] if "--only" in args else None
    gating = "--gate" in args
    record = "--record-baseline" in args
    path = os.path.join(HERE, "results.json")
    base_path = os.path.join(HERE, "baseline.json")
    cpu = cpu_name()
    base = None
    if gating:
        if not os.path.exists(base_path):
            sys.exit("no bench/showcase/baseline.json: record one on the gate's machine with --record-baseline")
        base = json.load(open(base_path))
        if (base.get("cpu"), base.get("machine")) != (cpu, machine()):
            print("baseline.json was taken on %s (%s), this is %s (%s): startup times are not comparable."
                  " Run the gate on the baseline's machine, or record a new baseline here." % (
                      base.get("cpu"), base.get("machine"), cpu, machine()))
            sys.exit(2)
    old = json.load(open(path)) if os.path.exists(path) else {"showcases": []}
    keep = {s["name"]: s for s in old.get("showcases", [])}
    fresh = {}
    work = tempfile.mkdtemp()
    try:
        by_name = {sc["name"]: sc for sc in SHOWCASES}
        names = [only] if only else (GATED if gating or record else [sc["name"] for sc in SHOWCASES])
        for name in names:
            if name not in by_name:
                sys.exit("no showcase %s (have: %s)" % (name, ", ".join(by_name)))
            d = tempfile.mkdtemp(dir=work)
            r = measure(by_name[name], runs, d)
            fresh[name] = r
            if "error" in r:
                print("%s: build failed: %s" % (name, r["error"]))
                continue
            print("%-12s brevi %7.1f ms  cpython %7.1f ms  %5.1fx  same output: %s  %s MB  needs python: %s" % (
                name, r["brevi"]["first_s"] * 1e3, r["cpython"]["first_s"] * 1e3, r["speedup"], r["same_output"], r["binary_mb"], r["needs_python"]))

        def remeasure(name):
            return measure(by_name[name], runs, tempfile.mkdtemp(dir=work))

        if gating:
            fails = gate(fresh, base, "--targets" in args, remeasure)
            if fails:
                print("\nsize/startup gate FAILED:\n  " + "\n  ".join(fails))
                sys.exit(1)
            print("\nsize/startup gate passed (baseline %s, %s)" % (base.get("commit"), base.get("date")))
            return
    finally:
        shutil.rmtree(work, ignore_errors=True)
    brevi_v = subprocess.run([BREVI, "--version"], capture_output=True, text=True).stdout.strip()
    commit = subprocess.run(["git", "-C", ROOT, "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip() or None
    head = {
        "date": time.strftime("%Y-%m-%d"), "brevi": brevi_v, "python": platform.python_version(), "machine": machine(),
        # the commit the Brevi binary was built from and the CPU, so a reader can place the numbers
        "commit": commit, "cpu": cpu,
    }
    if record:
        bad = [n for n in GATED if n not in fresh or "error" in fresh[n] or not fresh[n]["same_output"]]
        if bad:
            sys.exit("not recording a baseline: %s did not build or differ from CPython" % ", ".join(bad))
        out = dict(head, showcases={n: {"binary_bytes": fresh[n]["binary_bytes"], "first_s": fresh[n]["brevi"]["first_s"],
                                        "build_s": fresh[n]["build_s"], "build_peak_mb": fresh[n]["build_peak_mb"],
                                        "runs": fresh[n]["runs"]} for n in GATED})
        json.dump(out, open(base_path, "w"), indent=1)
        print("wrote", os.path.relpath(base_path, ROOT))
    keep.update(fresh)
    order = [s["name"] for s in SHOWCASES]
    out = dict(head, showcases=sorted(keep.values(), key=lambda s: order.index(s["name"]) if s["name"] in order else 99))
    json.dump(out, open(path, "w"), indent=1)
    print("wrote", os.path.relpath(path, ROOT))


if __name__ == "__main__":
    main()
