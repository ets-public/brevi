#!/usr/bin/env python3
"""Latency benchmark for Brevi.

Measures the agent inner loop in fresh subprocesses:
  check --fast  -> parse + infer + guard (no rustc)
  check         -> full check (rustc --emit=metadata)
  build         -> cold build with --no-cache (includes rustc)
  run           -> cached run of an unchanged program (result-cache replay)
  run --no-cache-> cold build + run (optional)

Usage:
  python3 bench/latency.py --make-fixtures bench/latency-fixtures/
  python3 bench/latency.py --brevi target/release/brevi --runs 20 --json latency.json
  python3 bench/latency.py --brevi target/release/brevi --runs 20 \
      --max-p50-check-fast 20 --max-p50-check 50 --max-p50-build 500 --max-p50-run 10

Exit code 1 when a --max-* threshold is exceeded, or when a command fails.
"""
from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

CHECK_TIMEOUT = 180


def percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    xs = sorted(values)
    k = (len(xs) - 1) * p
    lo = int(k)
    hi = min(lo + 1, len(xs) - 1)
    if lo == hi:
        return xs[lo]
    return xs[lo] + (xs[hi] - xs[lo]) * (k - lo)


def find_brevi(path_arg: str) -> str:
    p = Path(path_arg)
    candidates = [p]
    if sys.platform == "win32":
        candidates.append(p.with_suffix(".exe"))
        candidates.append(Path(str(p) + ".exe"))
    for c in candidates:
        if c.is_file():
            return str(c)
    found = shutil.which(path_arg)
    if found:
        return found
    sys.exit(f"error: brevi not found at {path_arg!r}; build it or pass --brevi")


def run_once(cmd: list[str], cwd: Path, timeout: int) -> tuple[float, subprocess.CompletedProcess]:
    t0 = time.perf_counter()
    proc = subprocess.run(
        cmd,
        cwd=str(cwd),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
    )
    return (time.perf_counter() - t0) * 1000.0, proc


def measure(
    label: str,
    cmd: list[str],
    cwd: Path,
    runs: int,
    warmups: int = 1,
    timeout: int = CHECK_TIMEOUT,
) -> dict:
    for _ in range(warmups):
        _, proc = run_once(cmd, cwd, timeout)
        if proc.returncode != 0:
            raise RuntimeError(
                f"{label}: command failed (exit {proc.returncode}): {' '.join(cmd)}\n"
                f"stderr:\n{proc.stderr.decode('utf-8', 'replace')[:4000]}"
            )
    samples: list[float] = []
    for _ in range(runs):
        ms, proc = run_once(cmd, cwd, timeout)
        if proc.returncode != 0:
            raise RuntimeError(
                f"{label}: command failed (exit {proc.returncode}): {' '.join(cmd)}\n"
                f"stderr:\n{proc.stderr.decode('utf-8', 'replace')[:4000]}"
            )
        samples.append(ms)
    return {
        "n": len(samples),
        "min_ms": round(min(samples), 2),
        "p50_ms": round(percentile(samples, 0.50), 2),
        "p95_ms": round(percentile(samples, 0.95), 2),
        "max_ms": round(max(samples), 2),
    }


def fixture_dirs(root: Path) -> list[Path]:
    if (root / "main.py").is_file():
        return [root]
    return sorted(p.parent for p in root.glob("*/main.py"))


def bench_fixture(brevi: str, fixture: Path, runs: int, include_cold_run: bool) -> dict:
    tmp = Path(tempfile.mkdtemp(prefix="brevi-latency-"))
    out = tmp / "prog"
    try:
        result = {
            "check_fast": measure("check_fast", [brevi, "check", "main.py", "--fast"], fixture, runs),
            "check": measure("check", [brevi, "check", "main.py"], fixture, runs),
            "build_cold": measure(
                "build_cold",
                [brevi, "build", "main.py", "-o", str(out), "--no-cache"],
                fixture,
                runs,
            ),
            "run_cached": measure("run_cached", [brevi, "run", "main.py"], fixture, runs),
        }
        if include_cold_run:
            result["run_cold"] = measure(
                "run_cold", [brevi, "run", "main.py", "--no-cache"], fixture, max(3, runs // 4)
            )
        return result
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


# ---------------------------------------------------------------- fixtures

UTIL = '''"""Utility helpers for the latency fixture (stdlib only)."""
import math


def clamp(x, lo, hi):
    if x < lo:
        return lo
    if x > hi:
        return hi
    return x


def is_prime(n):
    if n < 2:
        return False
    if n % 2 == 0:
        return n == 2
    i = 3
    while i * i <= n:
        if n % i == 0:
            return False
        i += 2
    return True


def norm(values):
    total = 0.0
    for v in values:
        total += v * v
    return math.sqrt(total)


def fib(n):
    a, b = 0, 1
    i = 0
    while i < n:
        a, b = b, a + b
        i += 1
    return a


def digits(n):
    out = []
    while n:
        out.append(n % 10)
        n //= 10
    out.reverse()
    return out
'''

WORK = '''"""Workload for the latency fixture."""
from util import clamp, is_prime, norm, fib, digits


def run_sieve(limit):
    flags = [True] * (limit + 1)
    flags[0] = False
    flags[1] = False
    p = 2
    while p * p <= limit:
        if flags[p]:
            m = p * p
            while m <= limit:
                flags[m] = False
                m += p
        p += 1
    return [i for i in range(limit + 1) if flags[i]]


def run_words(items):
    counts = {}
    for w in items:
        if w in counts:
            counts[w] = counts[w] + 1
        else:
            counts[w] = 1
    pairs = []
    for k in counts:
        pairs.append((k, counts[k]))
    pairs.sort()
    return pairs


def run_matrix(n):
    total = 0
    for i in range(n):
        for j in range(n):
            total += (i * j) % 7
    return total


def run_records(n):
    rows = []
    i = 0
    while i < n:
        rows.append({"id": i, "square": i * i, "even": i % 2 == 0})
        i += 1
    out = 0
    for r in rows:
        if r["even"]:
            out += r["square"]
    return out


def run_checks(n):
    hits = 0
    total = 0
    i = 0
    while i < n:
        v = clamp(i * 3, 0, n)
        if is_prime(v):
            hits += 1
        total += v
        i += 1
    return hits, total, norm([1.0, 2.0, 3.0]), fib(30), digits(123456789)
'''

MAIN = '''"""Latency fixture: ~200 lines across three files, stdlib only."""
import sys
from work import run_sieve, run_words, run_matrix, run_records, run_checks


def main():
    total = 0
    total += len(run_sieve(2000))
    total += len(run_words(["alpha", "beta", "alpha", "gamma", "beta", "alpha"]))
    total += run_matrix(40)
    total += run_records(500)
    hits, t, n, f, d = run_checks(3000)
    total += hits + t + int(n) + f + sum(d)
    print(total)
    return 0


if __name__ == "__main__":
    sys.exit(main())
'''

DYNAMIC_MAIN = '''"""Latency fixture for the dynamic path: values change type."""
import sys


def main():
    value = 1
    items = []
    i = 0
    while i < 500:
        if i % 3 == 0:
            value = "v" + str(i)
            items.append(value)
        elif i % 3 == 1:
            value = i * 2
            items.append(value)
        else:
            value = {"i": i}
            items.append(value)
        i += 1
    print(len(items), type(value).__name__)
    return 0


if __name__ == "__main__":
    sys.exit(main())
'''


def make_fixtures(root: Path) -> None:
    typed = root / "typed"
    dynamic = root / "dynamic"
    typed.mkdir(parents=True, exist_ok=True)
    dynamic.mkdir(parents=True, exist_ok=True)
    (typed / "util.py").write_text(UTIL, encoding="utf-8")
    (typed / "work.py").write_text(WORK, encoding="utf-8")
    (typed / "main.py").write_text(MAIN, encoding="utf-8")
    (dynamic / "main.py").write_text(DYNAMIC_MAIN, encoding="utf-8")
    print(f"wrote fixtures under {root}")


# ---------------------------------------------------------------- reporting

KEYS = ["check_fast", "check", "build_cold", "run_cached", "run_cold"]
MAX_FLAGS = {
    "check_fast": "max_p50_check_fast",
    "check": "max_p50_check",
    "build_cold": "max_p50_build",
    "run_cached": "max_p50_run",
}


def markdown(results: dict) -> str:
    lines = ["| fixture | metric | n | min ms | p50 ms | p95 ms | max ms |", "|---|---|---:|---:|---:|---:|---:|"]
    for fixture, metrics in results["fixtures"].items():
        for key in KEYS:
            if key not in metrics:
                continue
            m = metrics[key]
            lines.append(
                f"| {fixture} | {key} | {m['n']} | {m['min_ms']} | {m['p50_ms']} | {m['p95_ms']} | {m['max_ms']} |"
            )
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--brevi", default="target/release/brevi", help="path to the brevi binary")
    ap.add_argument("--fixtures", default="bench/latency-fixtures", help="fixture root (subdirs containing main.py)")
    ap.add_argument("--runs", type=int, default=20, help="samples per metric (default 20)")
    ap.add_argument("--json", dest="json_out", help="write raw results as JSON")
    ap.add_argument("--markdown", dest="md_out", help="write the markdown table to a file")
    ap.add_argument("--include-cold-run", action="store_true", help="also measure run --no-cache")
    ap.add_argument("--make-fixtures", metavar="DIR", help="write the fixture set to DIR and exit")
    ap.add_argument("--max-p50-check-fast", type=float, default=None)
    ap.add_argument("--max-p50-check", type=float, default=None)
    ap.add_argument("--max-p50-build", type=float, default=None)
    ap.add_argument("--max-p50-run", type=float, default=None)
    args = ap.parse_args()

    if args.make_fixtures:
        make_fixtures(Path(args.make_fixtures))
        return 0

    fixtures = fixture_dirs(Path(args.fixtures))
    if not fixtures:
        sys.exit(
            f"error: no fixtures with main.py under {args.fixtures!r}. "
            f"Run with --make-fixtures {args.fixtures} first."
        )

    brevi = find_brevi(args.brevi)
    results = {
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%Sz", time.gmtime()),
        "brevi": brevi,
        "platform": platform.platform(),
        "runs": args.runs,
        "fixtures": {},
    }
    try:
        for fixture in fixtures:
            name = fixture.name if fixture.name != args.fixtures else "main"
            print(f"measuring {fixture} ...", file=sys.stderr)
            results["fixtures"][name] = bench_fixture(brevi, fixture, args.runs, args.include_cold_run)
    except RuntimeError as exc:
        print(str(exc), file=sys.stderr)
        return 1

    table = markdown(results)
    print(table)
    if args.md_out:
        Path(args.md_out).write_text(table + "\n", encoding="utf-8")
    if args.json_out:
        Path(args.json_out).write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")

    failures = []
    for fixture, metrics in results["fixtures"].items():
        for key, flag in MAX_FLAGS.items():
            limit = getattr(args, flag)
            if limit is None or key not in metrics:
                continue
            p50 = metrics[key]["p50_ms"]
            if p50 > limit:
                failures.append(f"{fixture}.{key}: p50 {p50:.1f} ms > {limit:.1f} ms")
    if failures:
        print("\nTHRESHOLD FAILURES:", file=sys.stderr)
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
