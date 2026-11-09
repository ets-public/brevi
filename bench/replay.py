"""A/B test of check feedback: replay saved e2e conversations from their first failed check with different feedback.

For each conversation whose first check failed, the files are rebuilt from the model's own write_file/edit_file
calls up to that check, the check's result is replaced by the feedback under test, and the model carries on from
there as in e2e.py. Only the tokens spent after the branch point are counted, so the first attempt (which the
feedback cannot change) does not add noise.

usage: python3 replay.py OUT --from "work/e2e/sh1*-haiku-pyb/*" --feedback plain|diff [--samples 2] [--jobs 4]
       python3 replay.py --summary "logs/replay/*.jsonl"
"""
import argparse, concurrent.futures as cf, difflib, functools, glob, json, os, shutil, subprocess, sys, tempfile
from pathlib import Path

B = Path(__file__).resolve().parent
sys.path.insert(0, str(B))
import e2e  # noqa: E402

# Records which source line printed each stdout line (the attribution a Brevi-built binary can give for free).
TRACE = r"""
import builtins, linecache, os, runpy, sys
_print, _rec, _buf = builtins.print, [], [""]
def _where():
    f = sys._getframe(2)
    while f and not f.f_code.co_filename.endswith(".py"):
        f = f.f_back
    return f"{os.path.basename(f.f_code.co_filename)}:{f.f_lineno}: {linecache.getline(f.f_code.co_filename, f.f_lineno).strip()}" if f else "?"
def print(*a, sep=" ", end="\n", file=None, flush=False):
    if file not in (None, sys.stdout):
        return _print(*a, sep=sep, end=end, file=file, flush=flush)
    s = sep.join(map(str, a)) + end
    w = _where()
    parts = s.split("\n")
    for i in range(1, len(parts)):
        _rec.append(w if len(parts) == 2 else "")  # a print of several lines (a join) says nothing useful
    _print(*a, sep=sep, end=end, flush=flush)
builtins.print = print
sys.argv = [sys.argv[1]]
try:
    runpy.run_path(sys.argv[0], run_name="__main__")
finally:
    sys.stdout.flush()
    open(os.environ["TRACE_OUT"], "w").write("\n".join(_rec))
"""


def where_printed(work, inp):
    t = Path(tempfile.mkdtemp())
    (t / "trace.py").write_text(TRACE)
    env = dict(os.environ, TRACE_OUT=str(t / "rec"))
    try:
        subprocess.run([sys.executable, str(t / "trace.py"), "main.py"], stdin=open(inp), capture_output=True, text=True, timeout=10, cwd=work, env=env)
        return (t / "rec").read_text().split("\n")
    except (subprocess.TimeoutExpired, OSError):
        return []


def diff_feedback(task, work):
    """What `brevi test` prints: the input, then only the output lines that differ (with line numbers), and for the
    first wrong line the source line that printed it."""
    tdir = B / "tasks" / task
    plain, _ = e2e.run_check("python", task, work)
    if plain.startswith("PASS") or not plain.startswith("test "):
        return plain  # pass, missing files or a compile error: unchanged
    tests = sorted(glob.glob(str(tdir / "tests" / "*.in")))
    passed = 0
    first = None
    for t in tests:
        want = Path(t[:-3] + ".out").read_text().rstrip("\n").split("\n")
        got = subprocess.run([sys.executable, "main.py"], stdin=open(t), capture_output=True, text=True, timeout=10, cwd=work)
        out = (got.stdout + got.stderr).rstrip("\n").split("\n")
        if out == want:
            passed += 1
            continue
        if first:
            continue
        rec = where_printed(work, t)
        lines = [f"test {Path(t).stem} failed", "input:", Path(t).read_text().rstrip(), "output differences (- expected, + got, by output line):"]
        shown = False
        for op, a1, a2, b1, b2 in difflib.SequenceMatcher(None, want, out, autojunk=False).get_opcodes():
            if op == "equal":
                continue
            for i in range(a1, a2):
                lines.append(f"{i + 1:>3} - {want[i]}")
            for j in range(b1, b2):
                lines.append(f"{j + 1:>3} + {out[j]}")
                if not shown and j < len(rec) and rec[j]:
                    lines.append(f"      printed by {rec[j]}")
                    shown = True
        first = "\n".join(lines)
    return f"{first}\npassed {passed}/{len(tests)}"


def strip(c):
    # The saved conversation keeps fields the API does not take back (e.g. tool_use "caller").
    if isinstance(c, str):
        return c
    out = []
    for b in c:
        b = dict(b)
        b.pop("caller", None)
        if b.get("type") == "text":
            b.pop("citations", None)
        out.append(b)
    return out


def replay(client, src, feedback, sample, outdir):
    task = Path(src).name
    model = json.loads(open(glob.glob(str(B / "logs" / "e2e" / (Path(src).parent.name + ".jsonl")))[0]).readline())["model"]
    conv = json.load(open(Path(src) / "conversation.json"))
    work = outdir / f"{task}__{Path(src).parent.name}__{feedback}{sample}"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    # Walk the saved conversation turn by turn so assistant turns stay paired with their results.
    msgs = [conv[0]]
    k = 1
    branch = None
    while k < len(conv):
        a = conv[k]
        msgs.append({"role": "assistant", "content": strip(a["content"])})
        results = []
        check_at = None
        for b in a["content"]:
            if b["type"] != "tool_use":
                continue
            x = b["input"]
            if b["name"] == "write_file":
                (work / Path(x["path"]).name).write_text(x["content"])
                out = f"wrote {Path(x['path']).name}"
            elif b["name"] == "edit_file":
                p = work / Path(x["path"]).name
                text = p.read_text() if p.exists() else ""
                n = text.count(x["old"])
                out = f"edited {p.name}" if n == 1 else f"old text found {n} times in {p.name}; it must occur once"
                if n == 1:
                    p.write_text(text.replace(x["old"], x["new"]))
            elif b["name"] == "check":
                check_at = len(results)
                out = ""
            else:
                out = ""
            results.append({"type": "tool_result", "tool_use_id": b["id"], "content": out})
        if check_at is not None:
            plain, _ = e2e.run_check("python", task, work)
            if plain.startswith("PASS"):
                return None
            fb = plain if feedback == "plain" else diff_feedback(task, work)
            results[check_at]["content"] = fb
            results[check_at]["is_error"] = True
            msgs.append({"role": "user", "content": results})
            branch = fb
            break
        msgs.append({"role": "user", "content": strip(conv[k + 1]["content"])})
        k += 2
    if branch is None:
        return None
    r = e2e.solve(client(model), model, "pyb", task, work, "medium", start_messages=msgs, start_checks=1, tag_of=Path(src).parent.name)
    return {"src": str(Path(src).relative_to(B)), "task": task, "feedback": feedback, "sample": sample, "fb_chars": len(branch), **r}


def summary(pattern):
    rows = [json.loads(l) for f in sorted(glob.glob(pattern)) for l in open(f) if l.strip()]
    by = {}
    for r in rows:
        by.setdefault(r["feedback"], []).append(r)
    print(f"{'feedback':9} {'n':>3} {'pass':>4} {'checks':>6} {'output':>7} {'c.write':>8} {'usd':>8} {'fb chars':>8}")
    for fb, rs in sorted(by.items()):
        n = len(rs)
        print(f"{fb:9} {n:3} {sum(r['pass'] for r in rs):4} {sum(r['checks'] for r in rs) / n:6.2f} {sum(r['output'] for r in rs) / n:7.0f} {sum(r['cache_write'] for r in rs) / n:8.0f} {sum(r['usd'] for r in rs) / n:8.5f} {sum(r['fb_chars'] for r in rs) / n:8.0f}")
    # paired by source conversation
    srcs = sorted({r["src"] for r in rows})
    for s in srcs:
        line = [s]
        for fb in sorted(by):
            xs = [r for r in rows if r["src"] == s and r["feedback"] == fb]
            if xs:
                line.append(f"{fb} {sum(x['usd'] for x in xs) / len(xs):.5f} ({sum(x['checks'] for x in xs) / len(xs):.1f} checks)")
        print("  ", " | ".join(line))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out", nargs="?")
    ap.add_argument("--from", dest="src")
    ap.add_argument("--feedback", default="plain", help="comma-separated: plain,diff")
    ap.add_argument("--samples", type=int, default=2)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--summary")
    a = ap.parse_args()
    if a.summary:
        return summary(a.summary)
    # one client per model id (the source runs may mix Claude and DeepSeek); see e2e.make_client
    client = functools.lru_cache(maxsize=None)(e2e.make_client)
    outdir = B / "work" / "replay" / a.out
    log = B / "logs" / "replay" / f"{a.out}.jsonl"
    log.parent.mkdir(parents=True, exist_ok=True)
    jobs = [(s, fb, i) for s in sorted(glob.glob(str(B / a.src))) for fb in a.feedback.split(",") for i in range(a.samples)]
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        for f in cf.as_completed([ex.submit(replay, client, s, fb, i, outdir) for s, fb, i in jobs]):
            r = f.result()
            if r:
                with open(log, "a") as fh:
                    fh.write(json.dumps(r) + "\n")
                print(r["src"], r["feedback"], r["checks"], r["pass"], r["usd"], flush=True)
    summary(str(log))


if __name__ == "__main__":
    main()
