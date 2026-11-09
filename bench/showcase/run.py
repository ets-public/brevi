# Times the showcase programs built with Brevi against CPython running the same files.
#
#   python3 bench/showcase/run.py [--runs N] [--only NAME]
#
# Each showcase is built once with `brevi build --release`, then both sides run it N times (default 10) and the
# median is kept. Output must match between the two sides, or the showcase is reported as a mismatch.
# Results go to bench/showcase/results.json; the table on site/docs/showcases.md is written from it.
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


def links_python(binary):
    try:
        out = subprocess.run(["ldd", binary], capture_output=True, text=True).stdout
    except OSError:
        return None
    return "libpython" in out


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
    t0 = time.perf_counter()
    b = subprocess.run([BREVI, "build", src, "--release", "-o", exe], capture_output=True, text=True)
    build_s = time.perf_counter() - t0
    if b.returncode != 0:
        return {"name": sc["name"], "error": (b.stdout + b.stderr).strip()[-400:]}
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
        "binary_mb": round(os.path.getsize(exe) / 1e6, 1), "needs_python": links_python(exe),
    }


def main():
    runs = int(sys.argv[sys.argv.index("--runs") + 1]) if "--runs" in sys.argv else 10
    only = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else None
    path = os.path.join(HERE, "results.json")
    old = json.load(open(path)) if os.path.exists(path) else {"showcases": []}
    keep = {s["name"]: s for s in old.get("showcases", [])}
    work = tempfile.mkdtemp()
    try:
        for sc in SHOWCASES:
            if only and sc["name"] != only:
                continue
            r = measure(sc, runs, work)
            keep[sc["name"]] = r
            if "error" in r:
                print("%s: build failed: %s" % (sc["name"], r["error"]))
                continue
            print("%-12s brevi %7.1f ms  cpython %7.1f ms  %5.1fx  same output: %s  %s MB  needs python: %s" % (
                sc["name"], r["brevi"]["first_s"] * 1e3, r["cpython"]["first_s"] * 1e3, r["speedup"], r["same_output"], r["binary_mb"], r["needs_python"]))
    finally:
        shutil.rmtree(work, ignore_errors=True)
    brevi_v = subprocess.run([BREVI, "--version"], capture_output=True, text=True).stdout.strip()
    order = [s["name"] for s in SHOWCASES]
    out = {
        "date": time.strftime("%Y-%m-%d"), "brevi": brevi_v, "python": platform.python_version(),
        "machine": "%s %s, %d cores" % (platform.system(), platform.machine(), os.cpu_count() or 0),
        "showcases": sorted(keep.values(), key=lambda s: order.index(s["name"]) if s["name"] in order else 99),
    }
    json.dump(out, open(path, "w"), indent=1)
    print("wrote", os.path.relpath(path, ROOT))


if __name__ == "__main__":
    main()
