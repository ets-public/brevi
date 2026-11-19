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
CPython. It cannot run arbitrary shell commands, push, or touch the network. Everything its work can make run (the
compiler it edits, the programs it builds, its snippets, the check) runs in a box: its own PID, mount and network
namespaces, as user nobody, with an allowlisted environment, so no credential is reachable; the run refuses to
start if a self-check finds the box leaking (see tools/modelbox.py). When it calls
`finish`, the harness reruns the test (or check) itself and, if the change helps, commits it on a branch with
DeepSeek as the author. Nothing is pushed: a human (or Claude) reviews the branch, runs the wider tests, and opens the PR.

Spend is a hard stop (Adam 11-17). --budget is at most RUN_CAP ($2). Before each call the harness prices its worst
case (every prompt token a cache miss, a prompt-token bound from the request's bytes, max_tokens of output, peak
prices if peak could begin during the call) and lowers max_tokens so it fits under --budget, or stops the run when
no useful call fits. After the call it charges the API's own usage numbers; a reply whose usage is missing or does
not add up, or an attempt that may have been billed without a reply (timeout, dropped connection, 5xx, empty
body), is charged at its worst case, and an unreadable usage stops the run (fail closed). A lane (one test id, or
one --id / --check in task mode) gets RUNS_PER_LANE runs: the first and one resume. Every run is written to a
ledger on shared storage (--ledger, default DS_LEDGER or /mnt/project-files/deepseek/ledger.jsonl) before its
first call; a third run of a lane is refused (hand it to Opus), and so is any run when the ledger can't be read or
written. Every request, reply and tool result is logged as JSON lines to --log (default
bench/logs/ds_contrib/ID.jsonl, ID.run2.jsonl for the resume) so the run can be read afterwards. Needs
DEEPSEEK_API_KEY; stdlib only.
"""
import argparse, atexit, datetime, http.client, json, os, re, shlex, subprocess, sys, time, urllib.error, urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASELINE = ROOT / "compat" / "conformance_known_wrong"  # one file per entry, `test-id.mode` (tools/conformance.py)
KNOWN_DIRS = (BASELINE, ROOT / "compat" / "conformance_stdlib_known_wrong")
BASE_URL = os.environ.get("DEEPSEEK_BASE_URL", "https://api.deepseek.com")
# $ per million tokens: input (cache miss), cache hit, output. Peak prices; off-peak is half.
PRICES = {"deepseek-v4-pro": (1.32, 0.044, 3.96), "deepseek-flash": (0.30, 0.006, 1.20)}
# DeepSeek's peak hours, from its pricing page (checked 2026-11-12): 01:00-04:00 and 06:00-10:00 UTC, Monday to
# Friday; every other hour is off-peak at half price. Runs start only off-peak and stop when peak begins (Adam's
# standing rule); --allow-peak is for a run Adam has explicitly approved. Chinese public holidays are off-peak too,
# but are not known here, so they count as peak: the check errs towards waiting, never towards paying double.
PEAK_UTC = ((1, 4), (6, 10))


def is_peak(now=None):
    now = now or datetime.datetime.now(datetime.timezone.utc)
    return now.weekday() < 5 and any(a <= now.hour < b for a, b in PEAK_UTC)


def next_off_peak(now=None):
    t = (now or datetime.datetime.now(datetime.timezone.utc)).replace(minute=0, second=0, microsecond=0)
    while is_peak(t):
        t += datetime.timedelta(hours=1)
    return t.strftime("%H:%M UTC")


RUN_CAP = 2.00  # dollars: the most one run may spend, whatever --budget says (Adam 11-17)
RUNS_PER_LANE = 2  # the first run and one resume; after that the lane goes to Opus (Adam 11-17)
LEDGER = Path(os.environ.get("DS_LEDGER", "/mnt/project-files/deepseek/ledger.jsonl"))
MAX_TOKENS = 32000  # output tokens (reasoning included) asked for per call, lowered when the budget is short
MIN_TOKENS = 4000  # a call that can't afford this many output tokens is not worth making: the run stops instead
PROMPT_SLACK = 2000  # tokens of chat template around the messages and tools, which the request bytes don't count
CALL_TIMEOUT = 900  # seconds per HTTP attempt
CALL_WINDOW = datetime.timedelta(seconds=5 * CALL_TIMEOUT + 100)  # longest one chat() can take, retries included


class BudgetStop(Exception):
    """The run must stop now: the next call could pass the budget, or a call's cost could not be read."""


def prices(model, peak):
    return tuple(p if peak else p / 2 for p in PRICES[model])


def peak_during(start, end):
    """Whether any of [start, end] is in peak hours. Peak windows are hours apart and a call is shorter than the
    off-peak gap between them, so checking the two ends is enough."""
    return is_peak(start) or is_peak(end)


class Meter:
    """A run's spend, held under its budget call by call (see the module docstring)."""

    def __init__(self, model, budget):
        self.model, self.budget, self.spent = model, budget, 0.0
        self.last = None  # (prompt_tokens, request bytes) of the last call whose usage was read

    def prompt_bound(self, nbytes):
        # A token is at least one byte of the request text, and the JSON around each message is longer than the
        # template's tokens for it, so a new request's prompt is at most the last call's prompt tokens plus the bytes
        # appended since (the message list only grows). The first call is bounded by its whole body.
        base = nbytes if self.last is None else self.last[0] + max(0, nbytes - self.last[1])
        return base + PROMPT_SLACK

    def worst(self, nbytes, max_tokens, peak):
        pin, _, pout = prices(self.model, peak)
        return (self.prompt_bound(nbytes) * pin + max_tokens * pout) / 1e6

    def plan(self, nbytes, peak):
        """max_tokens for the next call such that its worst case keeps the run within budget; BudgetStop if none."""
        pin, _, pout = prices(self.model, peak)
        left = self.budget - self.spent - self.prompt_bound(nbytes) * pin / 1e6
        n = min(MAX_TOKENS, int(left * 1e6 / pout)) if left > 0 else 0
        if n < MIN_TOKENS:
            raise BudgetStop(f"budget: ${self.spent:.3f} of ${self.budget:.2f} spent, and the next call's worst case "
                             f"(${self.worst(nbytes, MIN_TOKENS, peak):.3f} with {MIN_TOKENS} output tokens) does not fit")
        return n

    def charge_unknown(self, nbytes, max_tokens, peak):
        """An attempt that may have been billed but gave no usage: charge the worst case it was planned on."""
        self.spent += self.worst(nbytes, max_tokens, peak)

    def charge(self, usage, nbytes, max_tokens, peak):
        """Charge one reply from the API's usage. Fails closed: usage that can't be read is charged at its worst
        case and stops the run, and so does usage beyond the bound the call was planned on."""
        bound = self.prompt_bound(nbytes)
        try:
            prompt, out = usage["prompt_tokens"], usage["completion_tokens"]
            hit = usage.get("prompt_cache_hit_tokens", 0)
            miss = usage.get("prompt_cache_miss_tokens", prompt - hit if type(hit) is int and type(prompt) is int else None)
            if not all(type(v) is int and v >= 0 for v in (prompt, out, hit, miss)) or hit + miss != prompt:
                raise ValueError
        except (KeyError, TypeError, ValueError):
            self.charge_unknown(nbytes, max_tokens, peak)
            raise BudgetStop(f"the API's usage for a reply could not be read ({str(usage)[:200]}): charged the call's "
                             f"worst case and stopped, since spend can no longer be counted")
        pin, phit, pout = prices(self.model, peak)
        self.spent += (miss * pin + hit * phit + out * pout) / 1e6
        self.last = (prompt, nbytes)
        if out > max_tokens or prompt > bound:
            raise BudgetStop(f"a reply used {prompt} prompt and {out} output tokens, past the {bound} and {max_tokens} "
                             f"the call was priced on: the cost bound no longer holds, so the run stops")
        if self.spent > self.budget:
            raise BudgetStop(f"spend ${self.spent:.3f} passed the ${self.budget:.2f} budget")


def ledger_runs(ledger, lane, check=None):
    """The runs of this lane already in the ledger (a run is written before its first call, so a killed run
    counts). Raises OSError or ValueError if the ledger can't be read: the caller refuses the run then."""
    if not ledger.exists():
        return []
    runs = []
    for i, line in enumerate(ledger.read_text().splitlines(), 1):
        if not line.strip():
            continue
        try:
            r = json.loads(line)
        except ValueError:
            raise ValueError(f"{ledger}:{i} is not JSON")
        if r.get("event") == "start" and (r.get("lane") == lane or (check and r.get("check") == check)):
            runs.append(r)
    return runs


def ledger_write(ledger, **rec):
    ledger.parent.mkdir(exist_ok=True)  # one level only: a missing shared mount is an error, not a new folder
    with ledger.open("a") as f:
        f.write(json.dumps({"t": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"), **rec}) + "\n")


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
    ("write_file", "Create or overwrite a file under src/, tests/, compat/ or docs/ (prefer replace for edits).",
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


EDITS = {"replace", "write_file"}
EXPLORE_NUDGE = 20  # calls before the first nudge to start editing (SYSTEM asks for a first edit by about then)

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


def safe_path(p, write=False):
    q = (ROOT / p).resolve()
    if q != ROOT and ROOT not in q.parents:
        raise ValueError("path outside the repository")
    rel = q.relative_to(ROOT).parts
    if rel and rel[0] in (".git", "target"):
        raise ValueError("that folder is off limits")
    if q == (ROOT / "tools" / "conformance.py").resolve() or q == Path(__file__).resolve() or any(
            q == d.resolve() or d.resolve() in q.parents for d in KNOWN_DIRS):
        raise ValueError("this file is part of the test harness and must not be edited")
    if q in PROTECT:
        raise ValueError("this file is part of the task's check and must not be edited")
    if write and (not rel or rel[0] not in WRITABLE):
        raise ValueError("only files under " + ", ".join(w + "/" for w in WRITABLE) + " may be edited")
    return q


# Everything the model can make run (its try_python code, the compiler it edits and rebuilds, the programs that
# compiler builds, the check) runs in tools/modelbox.py's box, never as this harness: no credential, other process,
# host socket or network is reachable there (see that module). Only the harness's own git commands and file reads
# and writes run outside. An env dump in a run once leaked a key (11-10).
sys.path.insert(0, str(ROOT / "tools"))
from modelbox import Box, chown_tree, redact  # noqa: E402

DSC = Path("/mnt/project-files/deepseek-contrib")
BOX = None
# The model may write only what the harness commits (git add below); everything else in the checkout (tools/, the
# build scripts, .github/, Cargo.toml) can run outside the box later, in a reviewer's or CI's hands.
WRITABLE = ("src", "tests", "compat", "docs")


def box_init():
    global BOX
    (ROOT / "target").mkdir(exist_ok=True)
    chown_tree(ROOT / "target")
    ro = [DSC] + [Path(x) for x in os.environ.get("DS_BOX_RO", "").split(":") if x]
    BOX = Box(ROOT, rw=[ROOT / "target", os.environ.get("BREVI_CACHE")], ro=ro)
    atexit.register(BOX.close)
    BOX.selftest()


def sh(cmd, timeout=900, env=None, cwd=ROOT):
    """Run a command the model's work can influence: always in the box."""
    return BOX.run(cmd, timeout=timeout, env=env, cwd=cwd)


def git(*args):
    """The harness's own git commands (repository config is not the model's to change: .git is off limits)."""
    r = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, timeout=300)
    return r.returncode, redact(r.stdout), redact(r.stderr)


def mkbox(prefix):
    return BOX.mkdir(prefix)


class Task:
    def __init__(self, test_id, python):
        self.id, self.python = test_id, python
        self.file = test_id.split(".")[0]

    def conformance(self):
        out = Path(mkbox("dsc-"))
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
    d = Path(mkbox("dsc-path-"))
    brevi = str(ROOT / "target" / "release" / "brevi")
    c, so, se = sh([brevi, "build", str(safe_path(path)), "-o", str(d / "prog")], timeout=600, env={"BREVI_PATH": "1"})
    lines = [l for l in (so + se).splitlines() if l.strip()]
    paths = [l for l in lines if l.startswith('{"path"')]
    if c != 0:
        return f"build failed (exit {c}):\n" + clip("\n".join(lines), 4000)
    return "\n".join(paths) or "typed (no path line: the typed build ran)"


def typed_rust_errors(path):
    src = safe_path(path)
    d = Path(mkbox("dsc-rs-"))
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
    d = Path(mkbox("dsc-try-"))
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
        p = safe_path(a["path"], write=True)
        t = p.read_text()
        n = t.count(a["old"])
        if n != 1:
            return f"error: old text found {n} times (must be exactly once)"
        p.write_text(t.replace(a["old"], a["new"], 1))
        return "ok"
    if name == "write_file":
        p = safe_path(a["path"], write=True)
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
        return clip(git("diff")[1] or "no changes")
    raise ValueError("unknown tool " + name)


def now_utc():
    return datetime.datetime.now(datetime.timezone.utc)


def chat(model, messages, tools, effort, meter, sleep=time.sleep):
    """One model reply, with every attempt priced and charged on `meter` (BudgetStop when one would not fit)."""
    for attempt in range(5):
        t0 = now_utc()
        est_peak = peak_during(t0, t0 + CALL_WINDOW)
        sized = {"model": model, "messages": messages, "tools": tools, "max_tokens": MAX_TOKENS, "reasoning_effort": effort}
        n = meter.plan(len(json.dumps(sized).encode()), est_peak)
        body = json.dumps({**sized, "max_tokens": n}).encode()
        req = urllib.request.Request(BASE_URL + "/chat/completions", data=body, headers={
            "Content-Type": "application/json", "Authorization": "Bearer " + os.environ["DEEPSEEK_API_KEY"]})
        try:
            with urllib.request.urlopen(req, timeout=CALL_TIMEOUT) as r:
                out = json.loads(r.read())
        except urllib.error.HTTPError as e:
            if e.code != 429:
                meter.charge_unknown(len(body), n, est_peak)  # a 5xx may come after the model ran; 429 is refused up front
            if e.code in (429, 500, 502, 503) and attempt < 4:
                sleep(2 ** attempt * 3); continue
            raise RuntimeError(f"HTTP {e.code}: {e.read()[:500]!r}")
        except (urllib.error.URLError, TimeoutError, ConnectionError, http.client.HTTPException, json.JSONDecodeError):
            meter.charge_unknown(len(body), n, est_peak)  # the request may have been served and billed
            if attempt < 4:
                sleep(2 ** attempt * 3); continue
            raise
        peak = peak_during(t0, now_utc())  # billed at the price in force (the plan above assumed the worse one)
        # the API now and then answers 200 with an empty or choice-less body: charge it and retry rather than crash
        if isinstance(out, dict) and out.get("choices"):
            meter.charge(out.get("usage"), len(body), n, peak)
            return out
        if isinstance(out, dict) and out.get("usage"):
            meter.charge(out["usage"], len(body), n, peak)
        else:
            meter.charge_unknown(len(body), n, est_peak)
        if attempt < 4:
            sleep(2 ** attempt * 3); continue
        raise RuntimeError(f"no reply from the model after 5 tries: {str(out)[:300]}")


def baseline_ids():
    return sorted({f.name.rpartition(".")[0] for f in BASELINE.iterdir() if f.name != "README.md" and not f.name.startswith(".")})


def boxed_main(cmd):
    """`ds_contrib.py --boxed CMD...`: the same as `tools/modelbox.py CMD...` (kept for reviewers' scripts)."""
    import modelbox
    sys.argv = [sys.argv[0]] + cmd
    return modelbox.main()


def main():
    if sys.argv[1:2] == ["--boxed"]:
        return boxed_main(sys.argv[2:])
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("test_id", nargs="?")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--model", default="deepseek-v4-pro", choices=sorted(PRICES))
    ap.add_argument("--effort", default="max", choices=["low", "high", "max"], help="DeepSeek reasoning effort (default %(default)s)")
    ap.add_argument("--budget", type=float, default=1.50,
                    help=f"stop before spending more than this many dollars, at most {RUN_CAP:.2f} (default %(default)s)")
    ap.add_argument("--ledger", type=Path, default=LEDGER, help="the shared run ledger (default %(default)s)")
    ap.add_argument("--steps", type=int, default=150, help="most model calls (default %(default)s)")
    ap.add_argument("--explore", type=int, default=50,
                    help="model calls allowed before the first edit; the run stops there without one (default %(default)s)")
    ap.add_argument("--python", default=os.environ.get("CONFORMANCE_PYTHON", "python3.13"))
    ap.add_argument("--branch", help="commit a successful fix on this new branch (default: no commit)")
    ap.add_argument("--log", type=Path)
    ap.add_argument("--allow-peak", action="store_true", help="run in DeepSeek's peak hours too (only with Adam's explicit go-ahead)")
    ap.add_argument("--note", help="review feedback on an earlier attempt, added to the task (text, or @file)")
    ap.add_argument("--task", type=Path, help="task mode: the brief, a text or markdown file")
    ap.add_argument("--id", help="task mode: a short name for the task (log file and commit message)")
    ap.add_argument("--check", help="task mode: the command (one string, split like a shell) whose exit 0 means done")
    ap.add_argument("--protect", action="append", default=[], help="task mode: a file the model must not edit (repeat)")
    a = ap.parse_args()
    if not 0 < a.budget <= RUN_CAP:  # also refuses nan
        ap.error(f"--budget must be more than 0 and at most {RUN_CAP:.2f} dollars (the per-run cap)")
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
    if is_peak() and not a.allow_peak:
        sys.exit(f"DeepSeek peak hours (01:00-04:00 and 06:00-10:00 UTC, Mon-Fri, double price): queue this run for "
                 f"{next_off_peak()} or later. --allow-peak only with Adam's explicit go-ahead.")
    check_key = a.check if task_mode else None
    try:
        earlier = ledger_runs(a.ledger, a.test_id, check_key)
        a.ledger.parent.mkdir(exist_ok=True)
        a.ledger.open("a").close()  # writable now, not only when the first call is about to go out
    except (OSError, ValueError) as e:
        sys.exit(f"the run ledger {a.ledger} can't be read or written ({e}): runs per lane can't be counted, so no run starts")
    if len(earlier) >= RUNS_PER_LANE:
        sys.exit(f"lane {a.test_id} already had {len(earlier)} DeepSeek runs (the first and one resume; ledger "
                 f"{a.ledger}): hand it to Opus with the review notes instead of running DeepSeek again")
    run_no = len(earlier) + 1
    if git("status", "--porcelain", "--untracked-files=no")[1].strip():
        sys.exit("the working tree has changes: start from a clean checkout")
    box_init()
    task = CheckTask(a.id, a.task.read_text(), shlex.split(a.check), a.python) if task_mode else Task(a.test_id, a.python)
    log_path = a.log or ROOT / "bench" / "logs" / "ds_contrib" / (a.test_id + (".jsonl" if run_no == 1 else f".run{run_no}.jsonl"))
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
    finished, stopped = None, None
    meter = Meter(a.model, a.budget)
    try:  # written before the first call, so a run killed mid-way still counts towards its lane
        ledger_write(a.ledger, event="start", lane=a.test_id, check=check_key, run=run_no, model=a.model, budget=a.budget)
    except OSError as e:
        sys.exit(f"the run ledger {a.ledger} can't be written ({e}): runs per lane can't be counted, so no run starts")
    atexit.register(lambda: ledger_write(a.ledger, event="end", lane=a.test_id, run=run_no, spent=round(meter.spent, 4),
                                         stopped=stopped, fixed=bool(result.get("fixed")) if result else None))
    result = None
    # Pace, held by the harness: a lane that read for 130 of its 150 calls (threading-1112 lane c, 11-12) ran
    # out of steps with an unfinished edit. Nudges at EXPLORE_NUDGE and 10 calls before --explore; at --explore
    # with no edit the run stops, so a vague brief costs a third of a run, not all of it.
    first_edit, nudged, reads = None, set(), {}
    for step in range(a.steps):
        if first_edit is None and step >= a.explore:
            print(f"[ds] no edit after {a.explore} calls: stopping. Sharpen the brief (name the files and "
                  f"functions to change) and rerun, or raise --explore", flush=True)
            note(event="explore_stop", step=step); break
        if is_peak() and not a.allow_peak:
            print(f"[ds] DeepSeek peak hours began: stopping; resume from {next_off_peak()} with --note", flush=True)
            note(event="peak_stop", step=step); break
        try:
            resp = chat(a.model, messages, tools, a.effort, meter)
        except BudgetStop as e:
            stopped = str(e)
            print(f"[ds] stopping: {e}", flush=True)
            note(event="budget_stop", step=step, spent=round(meter.spent, 4), reason=stopped); break
        u = resp["usage"]
        m = resp["choices"][0]["message"]
        msg = {"role": "assistant", "content": m.get("content") or ""}
        if m.get("reasoning_content"):
            msg["reasoning_content"] = m["reasoning_content"]
        if m.get("tool_calls"):
            msg["tool_calls"] = m["tool_calls"]
        messages.append(msg)
        note(event="reply", step=step, usage=u, spent=round(meter.spent, 4), content=msg["content"],
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
                if name in EDITS and first_edit is None and not out.startswith("error"):
                    first_edit = step
                if name == "read_file" and args:
                    key = (args.get("path"), args.get("start"), args.get("end"))
                    if key in reads:
                        out = (f"(You read this same range at call {reads[key]}: use what you learned then "
                               f"instead of reading it again.)\n" + out)
                    reads.setdefault(key, step)
            out = redact(out)
            short = json.dumps(args)[:120] if args else ""
            print(f"[ds] step {step} ${meter.spent:.3f} {name} {short}", flush=True)
            note(event="tool", name=name, args=args, result=out[:4000])
            messages.append({"role": "tool", "tool_call_id": tc["id"], "content": out})
        if first_edit is None and not finished:
            for at in (EXPLORE_NUDGE, a.explore - 10):
                if step + 1 >= at and at not in nudged:
                    nudged.add(at)
                    messages.append({"role": "user", "content": f"Note: {step + 1} calls and no edit yet. The run stops "
                                     f"at {a.explore} calls without one. Make your first edit now from what you have read."})
                    break
        left = a.steps - step - 1
        if left in (20, 8, 3) and not finished:
            messages.append({"role": "user", "content": f"Note: {left} model calls left. Finish your edit, build, "
                             f"{'run_check' if task_mode else 'check_test'}, then call finish."})
        if finished:
            break
    spent = meter.spent
    print(f"[ds] done after ${spent:.3f}; rebuilding and checking", flush=True)
    diff = git("diff")[1]
    if diff and not finished:
        # Out of steps or budget mid-edit: keep the work so a reviewer, or a rerun with --note, can pick it up.
        kept = log_path.with_suffix(".patch")
        kept.write_text(diff)
        nxt = ("apply it and rerun with --note describing what is left (off-peak only; this lane's last DeepSeek run)"
               if run_no < RUNS_PER_LANE else "this lane has had its DeepSeek resume, so finish it on Opus")
        print(f"[ds] unfinished: the diff is in {kept}; review it, or {nxt}", flush=True)
    built = run_tool("build", {}, task) == "ok" if diff else True
    after, after_text = task.results() if built else ({}, "build failed")
    builds = ("check",) if task_mode else ("normal", "dyn")
    fixed = [b for b in builds if before.get(b) != "matched" and after.get(b) == "matched"]
    broke = [b for b in builds if before.get(b) == "matched" and after.get(b) != "matched"]
    result = {"test": a.test_id, "model": a.model, "run": run_no, "spent": round(spent, 4), "stopped": stopped, "claimed_ok": bool(finished and finished.get("ok")),
              "summary": (finished or {}).get("summary"), "before": before, "after": after, "fixed": fixed, "broke": broke,
              "changed_files": git("diff", "--name-only")[1].split()}
    note(event="result", **result)
    print(json.dumps(result, indent=2))
    if a.branch and diff and fixed and not broke and built:
        if not task_mode:
            for b in fixed:
                (BASELINE / f"{a.test_id}.{b}").unlink(missing_ok=True)
        git("checkout", "-b", a.branch)
        git("add", "-A", "src", "tests", "compat", "docs")
        head = f"task: {a.id}" if task_mode else f"conformance: fix {a.test_id}"
        msg = (f"{head}\n\n{(finished or {}).get('summary') or ''}\n\n"
               f"Written by DeepSeek ({a.model}) through tools/ds_contrib.py; API cost ${spent:.2f}.")
        git("-c", "user.name=DeepSeek (via ds_contrib)", "-c", "user.email=noreply@deepseek.com", "commit", "-q", "-m", msg)
        print(f"[ds] committed on {a.branch}", flush=True)
    return 0 if fixed and not broke else 1


if __name__ == "__main__":
    sys.exit(main())
