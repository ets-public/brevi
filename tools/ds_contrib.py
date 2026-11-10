#!/usr/bin/env python3
"""DeepSeek as a contributor: give it one failing CPython conformance test and let it write the fix.

  python3 tools/ds_contrib.py TEST_ID [--model deepseek-v4-pro] [--budget 1.50] [--steps 60]
  python3 tools/ds_contrib.py --list            the wrong results still in the baseline
  python3 tools/ds_contrib.py --task TASK.md --id NAME --check "CMD ARGS" [--protect PATH]...
                                                any other task: TASK.md is the brief, CMD is how the harness
                                                tells done from not done (exit 0 = done)

The model works in this checkout through a few tools: read, search and edit files under the repository (never
.git or outside it), rebuild the compiler, rerun the one conformance test (in task mode: the task's check
command), see which build path Brevi takes for a file, and build a small Python snippet with Brevi next to
CPython. It cannot run arbitrary shell commands, push, or touch the network. When it calls
`finish`, the harness reruns the test (or check) itself and, if the change helps, commits it on a branch with
DeepSeek as the author. Nothing is pushed: a human (or Claude) reviews the branch, runs the wider tests, and opens the PR.

Spend is counted from the API's own usage numbers at the prices below and the run stops at --budget.
Every request, reply and tool result is logged as JSON lines to --log (default bench/logs/ds_contrib/ID.jsonl)
so the run can be read afterwards. Needs DEEPSEEK_API_KEY; stdlib only.
"""
import argparse, http.client, json, os, re, shlex, subprocess, sys, tempfile, time, urllib.error, urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASELINE = ROOT / "compat" / "conformance_known_wrong.txt"
BASE_URL = os.environ.get("DEEPSEEK_BASE_URL", "https://api.deepseek.com")
# $ per million tokens: input (cache miss), cache hit, output. Peak prices; off-peak is half.
PRICES = {"deepseek-v4-pro": (1.32, 0.044, 3.96), "deepseek-flash": (0.30, 0.006, 1.20)}
MAX_OUT = 14000  # characters of one tool result sent back to the model

SYSTEM = """You are contributing a fix to Brevi, a compiler written in Rust that takes plain Python and emits a native
binary through rustc. The repository is the current working directory. Read docs/ARCHITECTURE.md first if you need
the layout: parse -> type inference -> typed build (src/ codegen) or dynamic build (src/dynpy, a runtime of
dynamic values) -> rustc. A build that cannot be made correct must refuse with a clear error rather than give a
wrong answer: refusing is always better than printing something CPython would not print.

Your task: one CPython test (from CPython's own Lib/test, cut into a standalone script) builds with Brevi but does
not behave like CPython. Find the root cause in the compiler or runtime and fix it in general, not just for this
script. Do not special-case the test, do not edit tools/conformance.py, and do not edit the test script.

Rules:
- Keep the change small and focused on this one bug. Match the surrounding code style. No new dependencies.
- After editing Rust, call `build` and then `check_test`. Use `try_python` to test small variations, including
  cases near the fix, so you do not break nearby behaviour.
- Never hold a RefCell borrow (`d.borrow()`, `.borrow_mut()`) across anything that can run user code: `eq`, `hash`,
  `repr`, `call`, `add_h`, comparisons. User `__eq__`/`__repr__` may change the same dict or list, and a held
  borrow then panics. Copy what you need out first (`let items = d.borrow().items();`). A borrow taken in a
  `match` scrutinee or a `for` header lives for the whole match or loop.
- If the right fix is to refuse the program (compile error) instead of running it wrongly, that is acceptable.
- If you cannot find a correct fix, say so in `finish` with ok=false and what you learned. A wrong fix is worse
  than none.
- You have a limited number of tool calls (the harness tells you when few are left). Read what you need, then
  edit: a first edit by about your 20th call is a good pace. Do not re-read files you have already seen.
- When done, call `finish` with a short summary: the root cause, the fix, and what you tested."""

TASK_SYSTEM = """You are contributing a change to Brevi, a compiler written in Rust that takes plain Python and emits a
native binary through rustc. The repository is the current working directory. Read docs/ARCHITECTURE.md first if you
need the layout: parse -> type inference -> typed build (src/ codegen, fast, fixed types) or dynamic build (src/dynpy,
a runtime of dynamic values) -> rustc. A build that cannot be made correct must refuse (or fall back to the dynamic
build) rather than give a wrong answer: a program that prints something CPython would not print is the worst outcome.

The task is below. The harness judges it with a check command you can run with `run_check`; it reruns that command
itself after you finish, so make the check pass by fixing the real cause, in general, not by special-casing the
inputs the check uses.

Rules:
- Keep the change small and focused on this one task. Match the surrounding code style. No new dependencies.
- After editing Rust, call `build`, then `run_check`. Use `brevi_path` to see which path a file takes and why,
  `typed_rust_errors` when the typed Rust does not compile, and
  `try_python` to test small variations next to CPython, including cases near your change, so nearby behaviour
  does not break. Any program that now takes the typed build must print exactly what CPython prints.
- Never hold a RefCell borrow (`d.borrow()`, `.borrow_mut()`) across anything that can run user code: `eq`, `hash`,
  `repr`, `call`, comparisons. Copy what you need out first.
- Do not edit the check command's own files, the test harness, or expected outputs.
- If you cannot find a correct change, say so in `finish` with ok=false and what you learned. A wrong change is
  worse than none.
- You have a limited number of tool calls (the harness tells you when few are left). Read what you need, then
  edit: a first edit by about your 20th call is a good pace. Do not re-read files you have already seen.
- When done, call `finish` with a short summary: the cause, the change, and what you tested."""

TOOLS = [
    ("read_file", "Read a text file under the repository, with line numbers.",
     {"path": "string", "start": "integer (1-based, optional)", "end": "integer (optional)"}, ["path"]),
    ("search", "Search the repository with ripgrep (regex). Returns matching lines with file:line.",
     {"pattern": "string", "path": "string (optional sub-path, default src)", "context": "integer lines of context (optional)"}, ["pattern"]),
    ("list_dir", "List a directory under the repository.", {"path": "string"}, ["path"]),
    ("replace", "Replace exactly one occurrence of old with new in a file. old must match exactly, once.",
     {"path": "string", "old": "string", "new": "string"}, ["path", "old", "new"]),
    ("write_file", "Create or overwrite a file under the repository (prefer replace for edits).",
     {"path": "string", "content": "string"}, ["path", "content"]),
    ("build", "Rebuild the compiler (cargo build --release). Returns errors, or ok.", {}, []),
    ("check_test", "Rerun the failing conformance test against the current build, for the normal and the forced "
     "dynamic build. Returns each category and the first difference from CPython.", {}, []),
    ("run_check", "Task mode only: run the task's check command and show its exit code and output (exit 0 = done).", {}, []),
    ("brevi_path", "Build a Python file under the repository with the current Brevi and show the path it took "
     "(typed, dynamic, pybridge, python) and why: the one JSON line BREVI_PATH=1 prints.", {"path": "string"}, ["path"]),
    ("typed_rust_errors", "Emit the typed build's Rust for a Python file under the repository (brevi emit), compile it "
     "with rustc, and show each rustc error with the Rust lines around it and the Python line it came from (the "
     "nearest /*@L<n>*/ marker). Use it when brevi_path says the typed build's Rust did not compile.",
     {"path": "string"}, ["path"]),
    ("try_python", "Run a Python snippet under CPython 3.13 and as a Brevi native build (normal and forced dynamic), "
     "and show stdout, stderr and exit code of each.", {"code": "string"}, ["code"]),
    ("git_diff", "Show your changes so far (git diff).", {}, []),
    ("finish", "Stop. ok=true if you believe the fix is correct and complete.",
     {"ok": "boolean", "summary": "string"}, ["ok", "summary"]),
]


PROTECT = set()  # task mode: extra files the model must not edit (the check's own files)
TASK_ONLY, TEST_ONLY = {"run_check"}, {"check_test"}


def tool_schemas(task_mode=False):
    out = []
    for name, desc, props, req in TOOLS:
        if name in (TEST_ONLY if task_mode else TASK_ONLY):
            continue
        p = {}
        for k, v in props.items():
            t = v.split()[0]
            p[k] = {"type": t, "description": v}
        out.append({"type": "function", "function": {"name": name, "description": desc,
                    "parameters": {"type": "object", "properties": p, "required": req}}})
    return out


def clip(s, n=MAX_OUT):
    return s if len(s) <= n else s[: n // 2] + f"\n... [{len(s) - n} characters cut] ...\n" + s[-n // 2:]


def safe_path(p):
    q = (ROOT / p).resolve()
    if q != ROOT and ROOT not in q.parents:
        raise ValueError("path outside the repository")
    rel = q.relative_to(ROOT).parts
    if rel and rel[0] in (".git", "target"):
        raise ValueError("that folder is off limits")
    if q == (ROOT / "tools" / "conformance.py").resolve() or q == BASELINE.resolve() or q == Path(__file__).resolve():
        raise ValueError("this file is part of the test harness and must not be edited")
    if q in PROTECT:
        raise ValueError("this file is part of the task's check and must not be edited")
    return q


def sh(cmd, timeout=900, env=None, cwd=ROOT):
    try:
        r = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout,
                           env={**os.environ, **(env or {})})
        return r.returncode, r.stdout, r.stderr
    except subprocess.TimeoutExpired:
        return -1, "", f"timed out after {timeout}s"


class Task:
    def __init__(self, test_id, python):
        self.id, self.python = test_id, python
        self.file = test_id.split(".")[0]

    def conformance(self):
        out = Path(tempfile.mkdtemp(prefix="dsc-"))
        code, so, se = sh([sys.executable, "tools/conformance.py", "--files", self.file, "--filter", "^" + re.escape(self.id) + "$",
                           "--python", self.python, "--out", str(out), "--jobs", "1"], timeout=1200)
        try:
            rep = json.loads((out / "report.json").read_text())
        except Exception:
            return None, clip(so + se, 4000)
        return rep, ""

    def results(self):
        rep, err = self.conformance()
        if rep is None:
            return {}, "conformance run failed:\n" + err
        res, lines = {}, []
        for t in rep.get("tests", []):
            if t.get("id") != self.id:
                continue
            for build, m in (t.get("modes") or {}).items():
                res[build] = m.get("category")
                lines.append(f"{build} build: {m.get('category')} (path {m.get('path')})  {m.get('detail', '')}")
        if not res:
            lines.append("test not found in the report")
        return res, "\n".join(lines)

    def script(self):
        code, so, se = sh([sys.executable, "tools/conformance.py", "--files", self.file, "--python", self.python, "--show", self.id], timeout=300)
        return so if code == 0 else se


class CheckTask:
    """Task mode: the brief comes from a file and `done` is the exit code of a command the harness runs."""
    def __init__(self, name, brief, check, python):
        self.id, self.brief, self.check, self.python = name, brief, check, python

    def run_check(self):
        c, so, se = sh(self.check, timeout=3600)
        return c, f"exit {c}\n" + clip(so + se, 8000)

    def results(self):
        c, text = self.run_check()
        return {"check": "matched" if c == 0 else f"exit {c}"}, text


def brevi_path(path):
    d = Path(tempfile.mkdtemp(prefix="dsc-path-"))
    brevi = str(ROOT / "target" / "release" / "brevi")
    c, so, se = sh([brevi, "build", str(safe_path(path)), "-o", str(d / "prog")], timeout=600, env={"BREVI_PATH": "1"})
    lines = [l for l in (so + se).splitlines() if l.strip()]
    paths = [l for l in lines if l.startswith('{"path"')]
    if c != 0:
        return f"build failed (exit {c}):\n" + clip("\n".join(lines), 4000)
    return "\n".join(paths) or "typed (no path line: the typed build ran)"


def typed_rust_errors(path):
    src = safe_path(path)
    d = Path(tempfile.mkdtemp(prefix="dsc-rs-"))
    brevi = str(ROOT / "target" / "release" / "brevi")
    c, rust, se = sh([brevi, "emit", str(src)], timeout=300)
    if c != 0 or not rust.strip():
        return f"brevi emit failed (exit {c}):\n" + clip(se, 4000)
    (d / "prog.rs").write_text(rust)
    c, so, se = sh(["rustc", "--edition", "2021", "--error-format", "short", "-o", str(d / "prog"), "prog.rs"], timeout=600, cwd=d)
    if c == 0:
        return "the typed Rust compiles"
    lines = rust.splitlines()
    py = src.read_text(errors="replace").splitlines()
    out = []
    for e in [l for l in se.splitlines() if l.startswith("prog.rs:") and ": error" in l][:12]:
        n = int(e.split(":")[1])
        mark = next((re.search(r"/\*@L(\d+)\*/", lines[i]) for i in range(n - 1, -1, -1) if re.search(r"/\*@L(\d+)\*/", lines[i])), None)
        pl = int(mark.group(1)) if mark else 0
        ctx = "\n".join(f"  {i + 1}: {lines[i][:220]}" for i in range(max(0, n - 3), min(len(lines), n + 2)))
        out.append(f"{e}\n{ctx}\n  python line {pl}: {py[pl - 1].strip() if 0 < pl <= len(py) else '?'}")
    return clip("\n\n".join(out) or se, 12000)


def try_python(code, python):
    d = Path(tempfile.mkdtemp(prefix="dsc-try-"))
    (d / "case.py").write_text(code)
    parts = []
    c, so, se = sh([python, "case.py"], timeout=30, cwd=d)
    parts.append(f"== CPython: exit {c}\n{so}{se}")
    brevi = str(ROOT / "target" / "release" / "brevi")
    for label, env in (("brevi normal", {"BREVI_PATH": "1"}), ("brevi dynamic", {"BREVI_PATH": "1", "BREVI_DYN": "1"})):
        c, so, se = sh([brevi, "build", "case.py", "-o", "prog"], timeout=600, env=env, cwd=d)
        if c != 0:
            parts.append(f"== {label}: build failed (exit {c})\n{clip(so + se, 3000)}")
            continue
        c, so, se = sh(["./prog"], timeout=30, cwd=d)
        parts.append(f"== {label}: exit {c}\n{so}{se}")
    return clip("\n".join(parts))


def run_tool(name, a, task):
    if name == "read_file":
        p = safe_path(a["path"])
        lines = p.read_text(errors="replace").splitlines()
        s = max(1, int(a.get("start") or 1)); e = min(len(lines), int(a.get("end") or s + 399))
        return clip("\n".join(f"{i}\t{lines[i - 1]}" for i in range(s, e + 1)) + (f"\n[{len(lines)} lines in file]" if e < len(lines) else ""))
    if name == "search":
        p = safe_path(a.get("path") or "src")
        cmd = ["rg", "-n", "--no-heading", "-g", "!target", "-m", "60"]
        if a.get("context"):
            cmd += ["-C", str(min(8, int(a["context"])))]
        c, so, se = sh(cmd + ["-e", a["pattern"], str(p)], timeout=60)
        return clip(so.replace(str(ROOT) + "/", "") or se or "no matches")
    if name == "list_dir":
        p = safe_path(a["path"])
        return "\n".join(sorted(x.name + ("/" if x.is_dir() else "") for x in p.iterdir()))
    if name == "replace":
        p = safe_path(a["path"])
        t = p.read_text()
        n = t.count(a["old"])
        if n != 1:
            return f"error: old text found {n} times (must be exactly once)"
        p.write_text(t.replace(a["old"], a["new"], 1))
        return "ok"
    if name == "write_file":
        p = safe_path(a["path"])
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(a["content"])
        return "ok"
    if name == "build":
        c, so, se = sh(["cargo", "build", "--release", "--message-format", "short"], timeout=1800)
        if c == 0:
            return "ok"
        errs = [l for l in se.splitlines() if "error" in l or l.startswith(" ")]
        return "build failed:\n" + clip("\n".join(errs) or se, 8000)
    if name == "check_test":
        return task.results()[1]
    if name == "run_check":
        return task.run_check()[1]
    if name == "brevi_path":
        return brevi_path(a["path"])
    if name == "typed_rust_errors":
        return typed_rust_errors(a["path"])
    if name == "try_python":
        return try_python(a["code"], task.python)
    if name == "git_diff":
        return clip(sh(["git", "diff"])[1] or "no changes")
    raise ValueError("unknown tool " + name)


def chat(model, messages, tools, effort="max"):
    body = json.dumps({"model": model, "messages": messages, "tools": tools, "max_tokens": 32000,
                       "reasoning_effort": effort}).encode()
    for attempt in range(5):
        req = urllib.request.Request(BASE_URL + "/chat/completions", data=body, headers={
            "Content-Type": "application/json", "Authorization": "Bearer " + os.environ["DEEPSEEK_API_KEY"]})
        try:
            with urllib.request.urlopen(req, timeout=900) as r:
                return json.loads(r.read())
        except urllib.error.HTTPError as e:
            if e.code in (429, 500, 502, 503) and attempt < 4:
                time.sleep(2 ** attempt * 3); continue
            raise RuntimeError(f"HTTP {e.code}: {e.read()[:500]!r}")
        except (urllib.error.URLError, TimeoutError, ConnectionError, http.client.HTTPException):
            if attempt < 4:
                time.sleep(2 ** attempt * 3); continue
            raise


def cost(model, u):
    pin, phit, pout = PRICES[model]
    hit = u.get("prompt_cache_hit_tokens", 0) or 0
    miss = u.get("prompt_cache_miss_tokens", max(0, u.get("prompt_tokens", 0) - hit))
    return (miss * pin + hit * phit + u.get("completion_tokens", 0) * pout) / 1e6


def baseline_ids():
    return sorted({l.split()[0] for l in BASELINE.read_text().splitlines() if l.strip() and not l.startswith("#")})


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("test_id", nargs="?")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--model", default="deepseek-v4-pro", choices=sorted(PRICES))
    ap.add_argument("--effort", default="max", choices=["low", "high", "max"], help="DeepSeek reasoning effort (default %(default)s)")
    ap.add_argument("--budget", type=float, default=1.50, help="stop at this many dollars (default %(default)s)")
    ap.add_argument("--steps", type=int, default=150, help="most model calls (default %(default)s)")
    ap.add_argument("--python", default=os.environ.get("CONFORMANCE_PYTHON", "python3.13"))
    ap.add_argument("--branch", help="commit a successful fix on this new branch (default: no commit)")
    ap.add_argument("--log", type=Path)
    ap.add_argument("--note", help="review feedback on an earlier attempt, added to the task (text, or @file)")
    ap.add_argument("--task", type=Path, help="task mode: the brief, a text or markdown file")
    ap.add_argument("--id", help="task mode: a short name for the task (log file and commit message)")
    ap.add_argument("--check", help="task mode: the command (one string, split like a shell) whose exit 0 means done")
    ap.add_argument("--protect", action="append", default=[], help="task mode: a file the model must not edit (repeat)")
    a = ap.parse_args()
    if a.list:
        print("\n".join(baseline_ids())); return 0
    task_mode = a.task is not None
    if task_mode:
        if not (a.id and a.check):
            ap.error("--task needs --id and --check")
        PROTECT.update((ROOT / p).resolve() for p in a.protect)
        PROTECT.add(a.task.resolve())
        a.test_id = a.id
    elif not a.test_id:
        ap.error("give a test id (see --list), or --task")
    if sh(["git", "status", "--porcelain", "--untracked-files=no"])[1].strip():
        sys.exit("the working tree has changes: start from a clean checkout")
    task = CheckTask(a.id, a.task.read_text(), shlex.split(a.check), a.python) if task_mode else Task(a.test_id, a.python)
    log_path = a.log or ROOT / "bench" / "logs" / "ds_contrib" / (a.test_id + ".jsonl")
    log_path.parent.mkdir(parents=True, exist_ok=True)
    log = log_path.open("w")

    def note(**kw):
        log.write(json.dumps({"t": round(time.time(), 1), **kw}) + "\n"); log.flush()

    print(f"[ds] {a.test_id}: building this checkout, then measuring the failure", flush=True)
    if run_tool("build", {}, task) != "ok":  # a binary left by another checkout state would give a wrong "before"
        sys.exit("the checkout does not build")
    before, before_text = task.results()
    note(event="start", test=a.test_id, model=a.model, before=before, mode="task" if task_mode else "conformance")
    if task_mode:
        prompt = (f"Task {a.id}:\n\n{task.brief}\n\nCheck command (run by the harness, and by you with run_check): "
                  f"{a.check}\nIts result now:\n{before_text}")
    else:
        script = task.script()
        prompt = (f"Failing test: {a.test_id}\n\nCurrent result:\n{before_text}\n\nThe generated script and how to reproduce it:\n"
                  f"```python\n{clip(script, 12000)}\n```\nFind and fix the root cause.")
    if a.note:
        note_text = Path(a.note[1:]).read_text() if a.note.startswith("@") else a.note
        prompt += "\n\nA reviewer rejected an earlier fix for this test. Their notes, which your fix must respect:\n" + note_text
    messages = [{"role": "system", "content": TASK_SYSTEM if task_mode else SYSTEM}, {"role": "user", "content": prompt}]
    tools = tool_schemas(task_mode)
    spent, finished = 0.0, None
    for step in range(a.steps):
        if spent >= a.budget:
            print(f"[ds] budget ${a.budget:.2f} reached", flush=True); break
        resp = chat(a.model, messages, tools, a.effort)
        u = resp.get("usage", {})
        spent += cost(a.model, u)
        m = resp["choices"][0]["message"]
        msg = {"role": "assistant", "content": m.get("content") or ""}
        if m.get("reasoning_content"):
            msg["reasoning_content"] = m["reasoning_content"]
        if m.get("tool_calls"):
            msg["tool_calls"] = m["tool_calls"]
        messages.append(msg)
        note(event="reply", step=step, usage=u, spent=round(spent, 4), content=msg["content"],
             reasoning=(m.get("reasoning_content") or "")[:4000], calls=m.get("tool_calls"))
        if not m.get("tool_calls"):
            messages.append({"role": "user", "content": "Continue with a tool call, or call finish."})
            continue
        for tc in m["tool_calls"]:
            name = tc["function"]["name"]
            try:
                args = json.loads(tc["function"].get("arguments") or "{}")
            except ValueError:
                args = None
            if name == "finish" and args is not None:
                finished = args
                out = "ok"
            else:
                try:
                    out = run_tool(name, args, task) if args is not None else "error: arguments were not valid JSON"
                except Exception as e:
                    out = f"error: {e}"
            short = json.dumps(args)[:120] if args else ""
            print(f"[ds] step {step} ${spent:.3f} {name} {short}", flush=True)
            note(event="tool", name=name, args=args, result=out[:4000])
            messages.append({"role": "tool", "tool_call_id": tc["id"], "content": out})
        left = a.steps - step - 1
        if left in (20, 8, 3) and not finished:
            messages.append({"role": "user", "content": f"Note: {left} model calls left. Finish your edit, build, "
                             f"{'run_check' if task_mode else 'check_test'}, then call finish."})
        if finished:
            break
    print(f"[ds] done after ${spent:.3f}; rebuilding and checking", flush=True)
    diff = sh(["git", "diff"])[1]
    built = run_tool("build", {}, task) == "ok" if diff else True
    after, after_text = task.results() if built else ({}, "build failed")
    builds = ("check",) if task_mode else ("normal", "dyn")
    fixed = [b for b in builds if before.get(b) != "matched" and after.get(b) == "matched"]
    broke = [b for b in builds if before.get(b) == "matched" and after.get(b) != "matched"]
    result = {"test": a.test_id, "model": a.model, "spent": round(spent, 4), "claimed_ok": bool(finished and finished.get("ok")),
              "summary": (finished or {}).get("summary"), "before": before, "after": after, "fixed": fixed, "broke": broke,
              "changed_files": sh(["git", "diff", "--name-only"])[1].split()}
    note(event="result", **result)
    print(json.dumps(result, indent=2))
    if a.branch and diff and fixed and not broke and built:
        if not task_mode:
            lines = [l for l in BASELINE.read_text().splitlines() if not any(l == f"{a.test_id} {b}" for b in fixed)]
            BASELINE.write_text("\n".join(lines) + "\n")
        sh(["git", "checkout", "-b", a.branch])
        sh(["git", "add", "-A", "src", "tests", "compat", "docs"])
        head = f"task: {a.id}" if task_mode else f"conformance: fix {a.test_id}"
        msg = (f"{head}\n\n{(finished or {}).get('summary') or ''}\n\n"
               f"Written by DeepSeek ({a.model}) through tools/ds_contrib.py; API cost ${spent:.2f}.")
        sh(["git", "-c", "user.name=DeepSeek (via ds_contrib)", "-c", "user.email=noreply@deepseek.com", "commit", "-q", "-m", msg])
        print(f"[ds] committed on {a.branch}", flush=True)
    return 0 if fixed and not broke else 1


if __name__ == "__main__":
    sys.exit(main())
