"""End-to-end benchmark: every token a model spends to finish each task, through the Claude API.

Unlike run.sh (which scores code written plus errors read), this counts what the API bills: the system
prompt (the Brevi guide, cached), every tool call and result, the model's thinking and text, and every
retry, from each response's `usage`. It also prices each run in dollars, since output tokens cost 5x input.

Each task is a fresh conversation. The model gets the same tools in both languages:
  write_file(path, content)   edit_file(path, old, new)   check()   and for Brevi only  guide(word)
check() builds the program and runs the task's tests, printing PASS, the compiler errors, or the first
failing test, exactly as run.sh does. The run ends when the model stops calling tools or after 8 checks.

usage: python3 e2e.py RUN --lang brevi|python --model sonnet|haiku --set xl3 [--effort medium] [--jobs 3]
       python3 e2e.py --summary "logs/e2e/RUN*.jsonl"
Needs BENCH_ANTHROPIC_KEY and `pip install anthropic`. Writes logs/e2e/RUN-MODEL-LANG.jsonl, one line per task,
and keeps each task's files in work/e2e/RUN-MODEL-LANG/TASK/.
"""
import argparse, concurrent.futures as cf, glob, json, os, subprocess, sys, tempfile, time
from pathlib import Path

B = Path(__file__).resolve().parent
ROOT = B.parent
BREVI = os.environ.get("BREVI", str(ROOT / "target" / "release" / "brevi"))
MODELS = {"sonnet": "claude-sonnet-5-5", "haiku": "claude-haiku-5-5"}
# $ per million tokens: input, output, cache read; a 5-minute cache write is 1.25x input.
PRICES = {"claude-sonnet-5-5": (2.00, 10.00, 0.20), "claude-haiku-5-5": (0.10, 0.50, 0.01)}
MAX_CHECKS = 8

sys.path.insert(0, str(B))
from subject_prompt_sets import SETS  # noqa: E402


GUIDE = ROOT / "BREVI.md"


def system_prompt(lang):
    # "pyb": the model is told exactly what the Python runs are told; check builds its .py files with Brevi.
    lang = "python" if lang == "pyb" else lang
    name = {"brevi": "Brevi", "python": "Python 3 (standard library only)"}[lang]
    ext = {"brevi": "bv", "python": "py"}[lang]
    text = (
        f"You write small command-line programs in {name}. Programs read stdin and write stdout.\n"
        f"Write the solution with write_file as main.{ext} (or the file names the task asks for, with main.{ext} as the entry point). "
        "Call check to build it and run the task's tests: it prints PASS, the compiler errors, or the first failing test. "
        "When a check fails, change only what needs to change with edit_file, then check again. "
        "Stop calling tools once check prints PASS. Write code directly, without long planning notes."
    )
    if lang == "brevi":
        text += "\n\nThe Brevi language guide follows. For details it points to, call guide with a word.\n\n" + Path(GUIDE).read_text()
    return text


def tools(lang):
    lang = "python" if lang == "pyb" else lang
    ts = [
        {"name": "write_file", "description": "Write a whole file.", "input_schema": {"type": "object", "properties": {"path": {"type": "string"}, "content": {"type": "string"}}, "required": ["path", "content"], "additionalProperties": False}, "strict": True},
        {"name": "edit_file", "description": "Replace the one occurrence of old with new in a file.", "input_schema": {"type": "object", "properties": {"path": {"type": "string"}, "old": {"type": "string"}, "new": {"type": "string"}}, "required": ["path", "old", "new"], "additionalProperties": False}, "strict": True},
        {"name": "check", "description": "Build the program and run the task's tests.", "input_schema": {"type": "object", "properties": {}, "additionalProperties": False}, "strict": True},
    ]
    if lang == "brevi":
        ts.append({"name": "guide", "description": "Print the guide lines about a word or section.", "input_schema": {"type": "object", "properties": {"word": {"type": "string"}}, "required": ["word"], "additionalProperties": False}, "strict": True})
    return ts


def run_check(lang, task, work):
    """The same feedback as run.sh, without its logging."""
    ext = {"brevi": "bv", "python": "py", "pyb": "py"}[lang]
    tdir = B / "tasks" / task
    files = (tdir / "files").read_text().split() if (tdir / "files").exists() else ["main"]
    missing = [f"{f}.{ext}" for f in files if not (work / f"{f}.{ext}").exists()]
    if missing:
        return f"missing files: {' '.join(missing)}"
    main = work / f"main.{ext}"
    binp = Path(tempfile.mkdtemp()) / "prog"
    if lang == "pyb":
        # The same Python files, built by Brevi: each f.py is copied to f.bv so imports between files resolve.
        bdir = Path(tempfile.mkdtemp())
        for f in work.glob("*.py"):
            (bdir / (f.stem + ".bv")).write_text(f.read_text())
        r = subprocess.run([BREVI, "build", str(bdir / "main.bv"), "-o", str(binp)], capture_output=True, text=True, cwd=work)
        if r.returncode:
            return (r.stdout + r.stderr).strip().replace(".bv", ".py")
        cmd = [str(binp)]
    elif lang == "brevi":
        r = subprocess.run([BREVI, "build", str(main), "-o", str(binp)], capture_output=True, text=True, cwd=work)
        if r.returncode:
            return (r.stdout + r.stderr).strip()
        cmd = [str(binp)]
    else:
        for f in files:
            r = subprocess.run([sys.executable, "-m", "py_compile", f"{f}.py"], capture_output=True, text=True, cwd=work)
            if r.returncode:
                return (r.stdout + r.stderr).strip()
        cmd = [sys.executable, str(main)]
    tests = sorted(glob.glob(str(tdir / "tests" / "*.in")))
    passed, first = 0, None
    for t in tests:
        want = Path(t[:-3] + ".out").read_text()
        try:
            got = subprocess.run(cmd, stdin=open(t), capture_output=True, text=True, timeout=10, cwd=work)
            out = (got.stdout + got.stderr).rstrip("\n")
        except subprocess.TimeoutExpired:
            out = "timeout after 10 s"
        if out == want.rstrip("\n"):
            passed += 1
        elif first is None:
            first = f"test {Path(t).stem} failed\ninput:\n{Path(t).read_text().rstrip()}\nexpected:\n{want.rstrip()}\ngot:\n{out}"
    if passed == len(tests):
        return f"PASS {passed}/{len(tests)}"
    return f"{first}\npassed {passed}/{len(tests)}"


def solve(client, model, lang, task, work, effort, thinking="adaptive"):
    work.mkdir(parents=True, exist_ok=True)
    system = [{"type": "text", "text": system_prompt(lang), "cache_control": {"type": "ephemeral"}}]
    messages = [{"role": "user", "content": (B / "tasks" / task / "prompt.md").read_text()}]
    use = {"input": 0, "cache_write": 0, "cache_read": 0, "output": 0}
    checks, passed, calls, stop = 0, False, 0, ""
    while calls < 40:
        calls += 1
        for attempt in range(6):
            try:
                with client.messages.stream(
                    model=model, max_tokens=32000, system=system, tools=tools(lang), messages=messages,
                    output_config={"effort": effort}, cache_control={"type": "ephemeral"},
                    thinking={"type": "adaptive", "display": "summarized"} if thinking == "adaptive" else {"type": thinking},
                ) as s:
                    r = s.get_final_message()
                break
            except Exception as e:  # rate limits and overloads: back off and retry
                if attempt == 5:
                    raise
                time.sleep(2 ** attempt * 5)
        u = r.usage
        use["input"] += u.input_tokens
        use["cache_write"] += u.cache_creation_input_tokens or 0
        use["cache_read"] += u.cache_read_input_tokens or 0
        use["output"] += u.output_tokens
        stop = r.stop_reason
        messages.append({"role": "assistant", "content": r.content})
        if r.stop_reason != "tool_use":
            break
        results = []
        for b in r.content:
            if b.type != "tool_use":
                continue
            a, out, err = b.input, "", False
            try:
                if b.name == "write_file":
                    p = work / Path(a["path"]).name
                    p.write_text(a["content"])
                    out = f"wrote {p.name}"
                elif b.name == "edit_file":
                    p = work / Path(a["path"]).name
                    text = p.read_text()
                    n = text.count(a["old"])
                    if n != 1:
                        out, err = f"old text found {n} times in {p.name}; it must occur once", True
                    else:
                        p.write_text(text.replace(a["old"], a["new"]))
                        out = f"edited {p.name}"
                elif b.name == "check":
                    checks += 1
                    out = run_check(lang, task, work)
                    passed = out.startswith("PASS")
                    err = not passed
                elif b.name == "guide":
                    out = subprocess.run([BREVI, "guide", a["word"]], capture_output=True, text=True).stdout or "nothing found"
            except (OSError, KeyError) as e:
                out, err = str(e), True
            results.append({"type": "tool_result", "tool_use_id": b.id, "content": out, "is_error": err})
        messages.append({"role": "user", "content": results})
        if checks >= MAX_CHECKS and not passed:
            break
    # Keep the whole conversation (thinking, text, tool calls and results) for reading later.
    def plain(c):
        if isinstance(c, str):
            return c
        return [b if isinstance(b, dict) else b.model_dump(exclude_none=True) for b in c]
    (work / "conversation.json").write_text(json.dumps([{"role": m["role"], "content": plain(m["content"])} for m in messages], indent=1))
    pin, pout, pread = PRICES[model]
    cost = (use["input"] * pin + use["cache_write"] * pin * 1.25 + use["cache_read"] * pread + use["output"] * pout) / 1e6
    return {"task": task, "pass": passed, "checks": checks, "calls": calls, "stop": stop, **use, "usd": round(cost, 5)}


def summary(pattern):
    rows = {}
    for f in sorted(glob.glob(pattern)):
        rs = [json.loads(l) for l in open(f) if l.strip()]
        rows[Path(f).stem] = rs
    print(f"{'run':28} {'tasks':>5} {'pass':>4} {'1st':>3} {'input':>8} {'c.write':>8} {'c.read':>9} {'output':>8} {'all tok':>9} {'usd':>8}")
    for name, rs in rows.items():
        t = {k: sum(r[k] for r in rs) for k in ("input", "cache_write", "cache_read", "output", "usd")}
        first = sum(1 for r in rs if r["pass"] and r["checks"] == 1)
        total = t["input"] + t["cache_write"] + t["cache_read"] + t["output"]
        print(f"{name:28} {len(rs):5} {sum(r['pass'] for r in rs):4} {first:3} {t['input']:8} {t['cache_write']:8} {t['cache_read']:9} {t['output']:8} {total:9} {t['usd']:8.4f}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("run", nargs="?")
    ap.add_argument("--lang", choices=["brevi", "python", "pyb"], help="pyb: the Python prompt, built with Brevi")
    ap.add_argument("--model", choices=list(MODELS), default="sonnet")
    ap.add_argument("--set", default="xl3", choices=list(SETS))
    ap.add_argument("--tasks", help="comma-separated, instead of --set")
    ap.add_argument("--effort", default="medium", choices=["low", "medium", "high", "xhigh", "max"])
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--thinking", default="adaptive", choices=["adaptive", "between_tools"], help="between_tools turns thinking off (Sonnet)")
    ap.add_argument("--guide", help="a guide file to send instead of BREVI.md (for A/B tests)")
    ap.add_argument("--summary")
    a = ap.parse_args()
    if a.guide:
        global GUIDE
        GUIDE = Path(a.guide).resolve()
    if a.summary:
        return summary(a.summary)
    if not (a.run and a.lang):
        ap.error("RUN and --lang are needed")
    import anthropic
    client = anthropic.Anthropic(api_key=os.environ["BENCH_ANTHROPIC_KEY"], max_retries=4)
    model = MODELS[a.model]
    name = f"{a.run}-{a.model}-{a.lang}"
    tasks = a.tasks.split(",") if a.tasks else SETS[a.set]
    log = B / "logs" / "e2e" / f"{name}.jsonl"
    log.parent.mkdir(parents=True, exist_ok=True)
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(solve, client, model, a.lang, t, B / "work" / "e2e" / name / t, a.effort, a.thinking): t for t in tasks}
        for f in cf.as_completed(futs):
            r = {"run": name, "model": model, "lang": a.lang, "effort": a.effort, "thinking": a.thinking, **f.result()}
            with open(log, "a") as fh:
                fh.write(json.dumps(r) + "\n")
            print(json.dumps(r))
    summary(str(log))


if __name__ == "__main__":
    main()
