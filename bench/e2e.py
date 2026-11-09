"""End-to-end benchmark: every token a model spends to finish each task, through the Claude API.

Unlike run.sh (which scores code written plus errors read), this counts what the API bills: the system
prompt (the Brevi guide, cached), every tool call and result, the model's thinking and text, and every
retry, from each response's `usage`. It also prices each run in dollars, since output tokens cost 5x input.

Each task is a fresh conversation. The model gets the same tools in both languages:
  write_file(path, content)   edit_file(path, old, new)   check()   and for Brevi only  guide(word)
check() builds the program and runs the task's tests, printing PASS, the compiler errors, or the first
failing test, exactly as run.sh does. The run ends when the model stops calling tools or after 8 checks.

usage: python3 e2e.py RUN --lang brevi|python|pyb --model sonnet|haiku|deepseek|deepseek-pro --set rel [--effort medium] [--jobs 3] [--show-errors]
       python3 e2e.py --summary "logs/e2e/RUN*.jsonl"
       python3 e2e.py --verdict "logs/e2e/RUN[a-d]-*.jsonl"   (the release bar over several runs)
Claude needs BENCH_ANTHROPIC_KEY and `pip install anthropic`; DeepSeek needs DEEPSEEK_API_KEY (bench/deepseek.py,
standard library only). DeepSeek is report-only in --verdict for now (REPORT_ONLY). Writes logs/e2e/RUN-MODEL-LANG.jsonl, one line per task,
and keeps each task's files in work/e2e/RUN-MODEL-LANG/TASK/.
"""
import argparse, concurrent.futures as cf, glob, hashlib, json, os, subprocess, sys, tempfile, time
from pathlib import Path

B = Path(__file__).resolve().parent
ROOT = B.parent
BREVI = os.environ.get("BREVI", str(ROOT / "target" / "release" / "brevi"))
MODELS = {"sonnet": "claude-sonnet-5-5", "haiku": "claude-haiku-5-5", "deepseek": "deepseek-flash", "deepseek-pro": "deepseek-v4-pro"}
# $ per million tokens: input, output, cache read; a 5-minute cache write is 1.25x input (DeepSeek bills none).
# DeepSeek: peak list prices (off-peak is half; pyb and python run side by side, so the ratio is the same either way),
# as listed for deepseek-flash (V4.1 Flash) since 10 Sep 2026 and deepseek-v4-pro since 16 Aug 2026.
PRICES = {"claude-sonnet-5-5": (2.00, 10.00, 0.20), "claude-haiku-5-5": (0.10, 0.50, 0.01),
          "deepseek-flash": (0.30, 1.20, 0.006), "deepseek-v4-pro": (1.32, 3.96, 0.044)}
# Models that are run and reported but do not count toward the release bar yet. DeepSeek joins the bar once it has
# two weekly runs behind it (Nov 2026), so a provider's first-week surprises cannot fail a release.
REPORT_ONLY = {"deepseek-flash", "deepseek-v4-pro"}


def make_client(model):
    """The API client for a model id: Anthropic for Claude, bench/deepseek.py (OpenAI-compatible) for DeepSeek."""
    if model.startswith("deepseek"):
        if not os.environ.get("DEEPSEEK_API_KEY"):
            sys.exit("error: DEEPSEEK_API_KEY is not set")
        import deepseek
        return deepseek.Client(max_retries=4)
    try:
        import anthropic
    except ImportError:
        sys.exit("error: the anthropic package is missing; run `pip install anthropic` (needed for runs, not for --summary/--verdict)")
    if not os.environ.get("BENCH_ANTHROPIC_KEY"):
        sys.exit("error: BENCH_ANTHROPIC_KEY is not set")
    return anthropic.Anthropic(api_key=os.environ["BENCH_ANTHROPIC_KEY"], max_retries=4)
MAX_CHECKS = 8

sys.path.insert(0, str(B))
from subject_prompt_sets import SETS  # noqa: E402


GUIDE = ROOT / "BREVI.md"


# What Brevi's skill, MCP server and llms.txt tell an agent: its plain code runs natively, so it need not
# hand-optimise (on the speed set this halved Sonnet's output). pyb runs get it unless --no-hint.
NATIVE_HINT = "Your program is compiled to native code, which runs plain loops over ints and lists several times faster than CPython (1.7x to 39.5x, median 8.5x, on our 12 compute benchmarks). Write the plain, direct solution; do not hand-optimise it for speed."
HINT = True
# --show-errors (pyb): a check whose files Brevi cannot build still runs them with CPython, as `brevi run` does, but the
# model also sees why Brevi refused (the line `brevi run` prints on stderr). This is the loop an agent with stderr
# in view gets; the default arm hides the refusal and --native fails the check instead (docs/METRICS.md, WS4).
SHOW_ERRORS = False


def system_prompt(lang):
    # "pyb": the model is told what the Python runs are told, plus NATIVE_HINT; check builds its .py files with Brevi.
    hint = lang == "pyb" and HINT
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
    if hint:
        text += "\n\n" + NATIVE_HINT
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


# The path Brevi's last pyb build of each work dir took ("typed", "pybridge", "dynamic"; None when it did not
# build), from its {"path":...} line (BREVI_PATH=1).
LAST_PATH = {}
# Milliseconds the task's tests took to run in the last check that passed, by (lang, work dir): the speed of the
# program the model wrote (Brevi's native binary for pyb, CPython for python). Logged as run_ms from 7 Nov 2026.
LAST_RUN_MS = {}


def build_path(stderr):
    for line in stderr.splitlines():
        if line.startswith('{"path":'):
            try:
                return json.loads(line)["path"]
            except (ValueError, KeyError):
                pass
    return None


def run_check(lang, task, work, native_only=False):
    """The same feedback as run.sh, without its logging. Returns (feedback, native): for pyb, native says whether
    Brevi built the files; when it could not, the files run with CPython, as `brevi run x.py` does (unless native_only)."""
    ext = {"brevi": "bv", "python": "py", "pyb": "py"}[lang]
    tdir = B / "tasks" / task
    files = (tdir / "files").read_text().split() if (tdir / "files").exists() else ["main"]
    missing = [f"{f}.{ext}" for f in files if not (work / f"{f}.{ext}").exists()]
    if missing:
        return f"missing files: {' '.join(missing)}", None
    main = work / f"main.{ext}"
    binp = Path(tempfile.mkdtemp()) / "prog"
    if lang == "pyb":
        # The same Python files, built by Brevi as `brevi build main.py` would (typed build first, then the dynamic one).
        bdir = Path(tempfile.mkdtemp())
        for f in work.glob("*.py"):
            (bdir / f.name).write_text(f.read_text())
        r = subprocess.run([BREVI, "build", str(bdir / "main.py"), "-o", str(binp)], capture_output=True, text=True, cwd=work,
                           env=dict(os.environ, BREVI_PATH="1"))
        native = not r.returncode
        LAST_PATH[str(work)] = build_path(r.stderr) if native else None
        if not native and native_only:
            return (r.stdout + r.stderr).strip().replace(".bv", ".py"), False
        refusal = "" if native else (r.stdout + r.stderr).strip().replace(".bv", ".py")
        cmd = [str(binp)] if native else [sys.executable, str(main)]
    elif lang == "brevi":
        r = subprocess.run([BREVI, "build", str(main), "-o", str(binp)], capture_output=True, text=True, cwd=work)
        if r.returncode:
            return (r.stdout + r.stderr).strip(), None
        cmd = [str(binp)]
    else:
        for f in files:
            r = subprocess.run([sys.executable, "-m", "py_compile", f"{f}.py"], capture_output=True, text=True, cwd=work)
            if r.returncode:
                return (r.stdout + r.stderr).strip(), None
        cmd = [sys.executable, str(main)]
    # GUI tasks (tasks/NAME/gui): CPython runs tkinter programs through the headless stand-in in tkshim/.
    env = dict(os.environ, PYTHONPATH=str(B / "tkshim")) if (tdir / "gui").exists() and cmd[0] == sys.executable else None
    tests = sorted(glob.glob(str(tdir / "tests" / "*.in")))
    passed, first, ms = 0, None, 0.0
    for t in tests:
        want = Path(t[:-3] + ".out").read_text()
        try:
            t0 = time.perf_counter()
            got = subprocess.run(cmd, stdin=open(t), capture_output=True, text=True, timeout=10, cwd=work, env=env)
            ms += (time.perf_counter() - t0) * 1000
            out = (got.stdout + got.stderr).rstrip("\n")
        except subprocess.TimeoutExpired:
            out = "timeout after 10 s"
        if out == want.rstrip("\n"):
            passed += 1
        elif first is None:
            first = f"test {Path(t).stem} failed\ninput:\n{Path(t).read_text().rstrip()}\nexpected:\n{want.rstrip()}\ngot:\n{out}"
    native = native if lang == "pyb" else None
    note = ""
    if lang == "pyb" and SHOW_ERRORS and not native:
        note = "\n\nBrevi could not build this natively, so it ran with CPython (slower). Brevi said:\n" + refusal
    if passed == len(tests):
        LAST_RUN_MS[(lang, str(work))] = round(ms, 1)
        return f"PASS {passed}/{len(tests)}" + note, native
    return f"{first}\npassed {passed}/{len(tests)}" + note, native


def solve(client, model, lang, task, work, effort, thinking="adaptive", native_only=False, start_messages=None, start_checks=0, tag_of=None):
    """start_messages/start_checks: carry on from a saved conversation (replay.py); only later calls are counted."""
    work.mkdir(parents=True, exist_ok=True)
    # A short tag per run (e.g. e25a-sonnet-pyb) keeps each run's prompt cache to itself. Without it, pyb and python
    # share one prompt, so whichever ran first paid the cache writes and the other read them (before e25 that was
    # always pyb, about $0.007 per Sonnet task).
    tag = hashlib.sha1((tag_of or work.parent.name).encode()).hexdigest()[:8]
    system = [{"type": "text", "text": system_prompt(lang) + f"\n\nSession {tag}.", "cache_control": {"type": "ephemeral"}}]
    messages = start_messages or [{"role": "user", "content": (B / "tasks" / task / "prompt.md").read_text()}]
    use = {"input": 0, "cache_write": 0, "cache_read": 0, "output": 0}
    checks, passed, calls, stop, native = start_checks, False, 0, "", None
    # pyb only: for each check, whether CPython passes the same files (a False where pyb failed is a Brevi bug)
    shadow = []
    # pyb only: whether each check built natively, in order (natives[0] is the first-attempt native build)
    natives = []
    # tokens and dollars billed up to the call that followed the first PASS (tokens-to-green); None until then
    green = None
    started = time.time()
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
                    out, native = run_check(lang, task, work, native_only)
                    passed = out.startswith("PASS")
                    if lang == "pyb":
                        natives.append(bool(native))
                        shadow.append([passed, run_check("python", task, work)[0].startswith("PASS")])
                    err = not passed
                elif b.name == "guide":
                    out = subprocess.run([BREVI, "guide", a["word"]], capture_output=True, text=True).stdout or "nothing found"
            except (OSError, KeyError) as e:
                out, err = str(e), True
            results.append({"type": "tool_result", "tool_use_id": b.id, "content": out, "is_error": err})
        messages.append({"role": "user", "content": results})
        if passed and green is None:
            green = dict(use)
        if checks >= MAX_CHECKS and not passed:
            break
    # Keep the whole conversation (thinking, text, tool calls and results) for reading later.
    def plain(c):
        if isinstance(c, str):
            return c
        return [b if isinstance(b, dict) else b.model_dump(exclude_none=True) for b in c]
    (work / "conversation.json").write_text(json.dumps([{"role": m["role"], "content": plain(m["content"])} for m in messages], indent=1))
    pin, pout, pread = PRICES[model]
    price = lambda u: (u["input"] * pin + u["cache_write"] * pin * 1.25 + u["cache_read"] * pread + u["output"] * pout) / 1e6
    cost = price(use)
    # green counts every call up to the PASS; the closing reply after it is in usd but not in usd_to_green.
    togreen = {"tokens_to_green": sum(green.values()), "usd_to_green": round(price(green), 5)} if passed and green else {"tokens_to_green": None, "usd_to_green": None}
    pyb = {"shadow": shadow, "path": LAST_PATH.get(str(work)), "natives": natives, "first_native": natives[0] if natives else None, "errors_shown": SHOW_ERRORS} if lang == "pyb" else {}
    return {"task": task, "pass": passed, "checks": checks, "calls": calls, "stop": stop, "native": native, **pyb, **use, "usd": round(cost, 5), **togreen,
            "secs": round(time.time() - started, 1), "run_ms": LAST_RUN_MS.get((lang, str(work))) if passed else None}


def summary(pattern):
    rows = {}
    for f in sorted(glob.glob(pattern)):
        rs = [json.loads(l) for l in open(f) if l.strip()]
        rows[Path(f).stem] = rs
    print(f"{'run':28} {'tasks':>5} {'pass':>4} {'1st':>3} {'nat':>3} {'n1':>3} {'typ':>3} {'bug':>3} {'input':>8} {'c.write':>8} {'c.read':>9} {'output':>8} {'all tok':>9} {'usd':>8} {'usd2grn':>8}")
    for name, rs in rows.items():
        t = {k: sum(r[k] for r in rs) for k in ("input", "cache_write", "cache_read", "output", "usd")}
        first = sum(1 for r in rs if r["pass"] and r["checks"] == 1)
        total = t["input"] + t["cache_write"] + t["cache_read"] + t["output"]
        # nat: pyb tasks whose last check Brevi built natively (the rest ran with CPython); "-" for other languages
        nat = sum(1 for r in rs if r.get("native")) if any("native" in r and r["native"] is not None for r in rs) else "-"
        # typ: pyb tasks whose last check Brevi built on the typed path (typed or pybridge); "-" before paths were logged
        typ = sum(1 for r in rs if r.get("path") in ("typed", "pybridge")) if any("path" in r for r in rs) else "-"
        # n1: pyb tasks whose FIRST check Brevi built natively (first-attempt native); "-" before 9 Nov 2026
        n1 = sum(1 for r in rs if r.get("first_native")) if any("first_native" in r for r in rs) else "-"
        # usd2grn: dollars up to the first PASS, summed over tasks that passed (tokens-to-green); "-" before 9 Nov 2026
        g = [r["usd_to_green"] for r in rs if r.get("usd_to_green") is not None]
        g = f"{sum(g):8.4f}" if any("usd_to_green" in r for r in rs) else f"{'-':>8}"
        # bug: pyb checks that failed where CPython passed the same files (from e2e's shadow check, since s1a)
        bug = sum(1 for r in rs for p, c in r.get("shadow", []) if c and not p) if any("shadow" in r for r in rs) else "-"
        print(f"{name:28} {len(rs):5} {sum(r['pass'] for r in rs):4} {first:3} {nat:>3} {n1:>3} {typ:>3} {bug:>3} {t['input']:8} {t['cache_write']:8} {t['cache_read']:9} {t['output']:8} {total:9} {t['usd']:8.4f} {g}")


def verdict(pattern):
    """The release bar over every run matching pattern (Adam, 2 Nov 2026): every task passes, pyb costs within 10% of
    python for each model, and at least 90% of the final pyb programs are built natively. Also prints a rough noise
    figure: the spread of the cost ratio when tasks are resampled (a bootstrap over the run's tasks)."""
    import random
    files = sorted(glob.glob(pattern))
    rows = [json.loads(l) for f in files for l in open(f) if l.strip()]
    if not rows:
        # an empty glob must never read as a pass
        sys.exit(f"error: no run logs match {pattern!r}" + ("" if files else " (no files)") + "; nothing to check")
    ok = True
    for model in sorted({r["model"] for r in rows}):
        rs = [r for r in rows if r["model"] == model]
        side = {lang: [r for r in rs if r["lang"] == lang] for lang in ("pyb", "python")}
        if not side["pyb"] or not side["python"]:
            print(f"{model}: needs both pyb and python runs"); ok = ok and model in REPORT_ONLY; continue
        runs = sorted({r["run"].split("-")[0] for r in side["pyb"]})
        usd = {lang: sum(r["usd"] for r in side[lang]) / len(runs) for lang in side}
        ratio = usd["pyb"] / usd["python"]
        fails = {lang: sum(not r["pass"] for r in side[lang]) for lang in side}
        nat = sum(1 for r in side["pyb"] if r.get("native"))
        # bootstrap: resample (run, task) pairs, keeping each pair's pyb and python cost together
        cost = {}
        for r in rs:
            cost.setdefault((r["run"].split("-")[0], r["task"]), {})[r["lang"]] = r["usd"]
        pairs = [v for v in cost.values() if len(v) == 2]
        rnd, sims = random.Random(1), []
        for _ in range(2000):
            pick = [rnd.choice(pairs) for _ in pairs]
            sims.append(sum(p["pyb"] for p in pick) / sum(p["python"] for p in pick))
        sims.sort()
        lo, hi = sims[50], sims[-51]
        good = fails["pyb"] == 0 and ratio <= 1.10 and nat >= 0.9 * len(side["pyb"])
        report_only = model in REPORT_ONLY
        if not report_only:
            ok &= good
        print(f"{model}: {len(runs)} runs, {len(side['pyb'])} pyb tasks, pyb ${usd['pyb']:.4f} / python ${usd['python']:.4f} per run = {ratio:.3f}x "
              f"(95% range {lo:.2f}x to {hi:.2f}x), failed pyb {fails['pyb']} python {fails['python']}, native {nat}/{len(side['pyb'])} "
              f"({100 * nat / len(side['pyb']):.0f}%): {'meets the bar' if good else 'misses the bar'}{' (report only, not part of the release test yet)' if report_only else ''}")
        # Reported, not yet part of the bar (docs/METRICS.md): first-attempt native and tokens-to-green.
        if any("first_native" in r for r in side["pyb"]):
            n1 = sum(1 for r in side["pyb"] if r.get("first_native"))
            print(f"  first-attempt native {n1}/{len(side['pyb'])} ({100 * n1 / len(side['pyb']):.0f}%), retries per pyb task {sum(r['checks'] - 1 for r in side['pyb']) / len(side['pyb']):.2f}"
                  f" (python {sum(r['checks'] - 1 for r in side['python']) / len(side['python']):.2f})")
        g = {lang: [r for r in side[lang] if r.get("usd_to_green") is not None] for lang in side}
        both = {(r["run"].split("-")[0], r["task"]) for r in g["pyb"]} & {(r["run"].split("-")[0], r["task"]) for r in g["python"]}
        if both:
            tg = {lang: sum(r["usd_to_green"] for r in g[lang] if (r["run"].split("-")[0], r["task"]) in both) for lang in g}
            tt = {lang: sum(r["tokens_to_green"] for r in g[lang] if (r["run"].split("-")[0], r["task"]) in both) for lang in g}
            shown = " (errors shown)" if all(r.get("errors_shown") for r in g["pyb"]) else ""
            print(f"  tokens-to-green{shown} over {len(both)} tasks both passed: {tt['pyb'] / tt['python']:.3f}x tokens, {tg['pyb'] / tg['python']:.3f}x dollars")
    print("release test PASSED" if ok else "release test not passed")
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("run", nargs="?")
    ap.add_argument("--lang", choices=["brevi", "python", "pyb"], help="pyb: the Python prompt, built with Brevi")
    ap.add_argument("--model", choices=list(MODELS), default="sonnet")
    ap.add_argument("--set", default="rel", choices=list(SETS))
    ap.add_argument("--tasks", help="comma-separated, instead of --set")
    ap.add_argument("--effort", default="medium", choices=["low", "medium", "high", "xhigh", "max"])
    ap.add_argument("--jobs", type=int, default=3)
    ap.add_argument("--thinking", default="adaptive", choices=["adaptive", "between_tools"], help="between_tools turns thinking off (Sonnet)")
    ap.add_argument("--guide", help="a guide file to send instead of BREVI.md (for A/B tests)")
    ap.add_argument("--no-hint", action="store_true", help="pyb: leave out the native-code line (the prompt is then exactly the Python one)")
    ap.add_argument("--native", action="store_true", help="pyb: report Brevi build errors instead of running with CPython (the strict check used before e22)")
    ap.add_argument("--show-errors", action="store_true", help="pyb: when Brevi cannot build the files, run them with CPython but show the model why (the errors-shown arm)")
    ap.add_argument("--summary")
    ap.add_argument("--verdict", help="log glob: check the release bar over these runs (e.g. \"logs/e2e/w1[a-d]-*.jsonl\")")
    a = ap.parse_args()
    if a.guide:
        global GUIDE
        GUIDE = Path(a.guide).resolve()
    if a.no_hint:
        global HINT
        HINT = False
    if a.show_errors:
        global SHOW_ERRORS
        SHOW_ERRORS = True
    if a.summary:
        return summary(a.summary)
    if a.verdict:
        return verdict(a.verdict)
    if not (a.run and a.lang):
        ap.error("RUN and --lang are needed")
    model = MODELS[a.model]
    client = make_client(model)
    name = f"{a.run}-{a.model}-{a.lang}"
    tasks = a.tasks.split(",") if a.tasks else SETS[a.set]
    log = B / "logs" / "e2e" / f"{name}.jsonl"
    log.parent.mkdir(parents=True, exist_ok=True)
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        futs = {ex.submit(solve, client, model, a.lang, t, B / "work" / "e2e" / name / t, a.effort, a.thinking, a.native): t for t in tasks}
        for f in cf.as_completed(futs):
            r = {"run": name, "model": model, "lang": a.lang, "effort": a.effort, "thinking": a.thinking, **f.result()}
            with open(log, "a") as fh:
                fh.write(json.dumps(r) + "\n")
            print(json.dumps(r))
    summary(str(log))


if __name__ == "__main__":
    sys.exit(main())
