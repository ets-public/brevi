#!/usr/bin/env python3
"""Runs tests/check.sh in parallel jobs and prints the same report, in the same order.

check.sh runs ~1,000 programs one after another, and most of its time is rustc building one test program on one
core. This script cuts check.sh into the sections that start at each `#== NAME` line and runs them side by side.
A section marked `#== NAME split` runs its first top-level `for f in ...; do` loop as one job per file (with the
section's lines before the loop in front of each job, and its lines after the loop as one more job).

Each job gets its own folder for the scratch files check.sh writes to /tmp, and as its TMPDIR (where brevi keeps
a file's build), so jobs never share one. The HTTP and
LLM mock servers are started once here (CHECK_SERVERS=1 tells check.sh not to start its own), so the per-file
jobs that talk to them can run side by side.

Env CHECK_ONLY=NAME,... runs only those sections; CHECK_SKIP=NAME,... leaves those out.
usage: check_parallel.py [--update] [--jobs N] [--quiet] [--changed-since REF]
  --quiet               leave out the "ok" lines
  --changed-since REF   the fast tier for a branch (tools/ci_local.sh uses it): skip the sections for Brevi's
                        copies of Python libraries (mcp, openai, httpx, polars, ...) that nothing changed since
                        REF can affect. A change to the shared engine or runtime runs them all. CI and releases
                        run everything.     (Env: BREVI, as for check.sh; JOBS, default CPU count)
Exits 1 if any check failed, as check.sh does. Run from the repo root.
"""
import concurrent.futures as cf, contextlib, glob, os, re, shutil, subprocess, sys, tempfile, threading, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CHECK = os.path.join(ROOT, "tests", "check.sh")
# Mock servers that sections of check.sh talk to: (command, port, line that says it is up).
SERVERS = [(["python3", "tests/http/server.py", "18080"], "ready"), (["python3", "tests/llm/server.py", "18090"], "listening")]


# The sections that test one of Brevi's library copies: the modules in src/dynpy/py they test, and the files of
# the dynamic runtime only those libraries use. Any other change to src/, runtime/ or Cargo runs them all.
LIB_SECTIONS = {
    "numpy": (["numpy"], r"src/dynpy/rt_np"),
    "tiktoken": (["tiktoken"], r"src/dynpy/(rt_tok|tok\.rs)"),
    "tokenizers": (["tokenizers"], r"src/dynpy/(rt_hf|hf\.rs)"),
    "pydantic": (["pydantic"], r"src/dynpy/pyd\.rs"),
    "jinja2": (["jinja2", "markupsafe"], None),
    "polars": (["polars"], None),
    "sqlite3": (["sqlite3"], r"src/dynpy/(sqlglue|rt_db)"),
    "duckdb": (["duckdb"], r"src/dynpy/(duckglue|rt_db)"),
    "safetensors": (["safetensors"], r"src/dynpy/rt_st"),
    "onnxruntime": (["onnxruntime"], r"src/dynpy/(ortglue|rt_ort)"),
    "http": (["requests", "httpx"], r"src/dynpy/(netglue|rt_net)"),
    "llm": (["anthropic", "openai"], r"src/dynpy/(netglue|rt_net)"),
    "mcp_sdk": (["mcp"], r"src/dynpy/(srvglue|stdioglue)"),
    "mcp_http": (["mcp"], r"src/dynpy/(srvglue|stdioglue|netglue|rt_net)"),
}
LIB_SECTIONS["bv_libs"] = LIB_SECTIONS["bv_libs_sandbox"] = (["jinja2", "markupsafe", "numpy", "sqlite3", "tiktoken", "polars"], r"src/dynpy/(rt_np|rt_tok|tok\.rs|sqlglue|rt_db)")
# Files of src/ the dynamic build never reads (the typed path, Brevi's own MCP server, the GUI): changing only these
# leaves the library sections as they were (bv_libs aside: a .bv file goes through the typed front end too).
TYPED_ONLY = {f"src/{n}.rs" for n in "infer emit moves records prelude bigint scoped gui gui_glue mcp mcp_http verify".split()}
PY = "src/dynpy/py/"


def lib_of(name):
    """The library a file in src/dynpy/py belongs to (jinja2_ext.py -> jinja2, _brevi_mcp_types.py -> mcp)."""
    name = name.removesuffix(".py").removeprefix("_brevi_")
    return name.split("_")[0].split(".")[0]


def lib_closure(libs):
    """The libraries `libs` import, theirs included."""
    files = {}
    for f in os.listdir(os.path.join(ROOT, PY)):
        if f.endswith(".py"):
            files.setdefault(lib_of(f), []).append(f)
    seen, todo = set(), list(libs)
    while todo:
        m = todo.pop()
        if m in seen or m not in files:
            seen.add(m)
            continue
        seen.add(m)
        for f in files[m]:
            for imp in re.findall(r"^\s*(?:from|import)\s+([\w.]+)", open(os.path.join(ROOT, PY, f)).read(), re.M):
                todo.append(lib_of(imp))
    return seen


def skipped_sections(ref):
    """Library sections that nothing changed since `ref` (committed, staged, unstaged or untracked) can affect."""
    git = lambda *a: subprocess.run(["git", *a], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split("\n")
    try:
        base = git("merge-base", "HEAD", ref)[0]
    except subprocess.CalledProcessError:
        return set(), f"no merge base with {ref}"
    return skipped_for({f for f in git("diff", "--name-only", base) + git("ls-files", "--others", "--exclude-standard") if f})


def skipped_for(changed):
    libs = set(l for v in LIB_SECTIONS.values() for l in v[0])
    owned = lambda f: (f.startswith(PY) and lib_of(os.path.basename(f)) in libs) or any(
        p and re.match(p, f) for _, p in LIB_SECTIONS.values())
    for f in changed:
        if f in ("tests/check.sh", "tests/check_parallel.py") or f.startswith(("runtime/", "Cargo.")) or (
                f.startswith("src/") and f not in TYPED_ONLY and not owned(f)):
            return set(), f"{f} is shared by every library copy"  # the shared engine or runtime
    skip = set()
    for sec, (mods, glue) in LIB_SECTIONS.items():
        reach = lib_closure(mods)
        hit = any((f.startswith(PY) and lib_of(os.path.basename(f)) in reach) or (glue and re.match(glue, f))
                  or f.startswith("tests/" + {"mcp_http": "mcp_sdk", "bv_libs_sandbox": "bv_libs"}.get(sec, sec) + "/")
                  or (sec == "bv_libs" and f.startswith("tests/bv_py/"))
                  or (sec.startswith("bv_libs") and f.startswith("src/")) for f in changed)
        if not hit:
            skip.add(sec)
    return skip, None


def sections(lines):
    """[(name, split, [lines])]: the preamble (before the first marker) has name None."""
    out = [(None, False, [])]
    for l in lines:
        m = re.match(r"#== (\S+)( split)?\s*$", l)
        if m:
            out.append((m.group(1), bool(m.group(2)), []))
        out[-1][2].append(l)
    return out


def loop_end(body, i):
    """Index of the `done` at column 0 that closes the top-level loop starting at body[i]."""
    for j in range(i + 1, len(body)):
        if body[j].rstrip() == "done":
            return j
    raise SystemExit(f"check_parallel: no `done` closes {body[i].strip()!r}")


def jobs_of(name, split, body):
    """[(label, script lines)] for one section."""
    if not split:
        return [(name, body)]
    i = next((k for k, l in enumerate(body) if re.match(r"for f in .*; do\s*$", l)), None)
    if i is None:
        raise SystemExit(f"check_parallel: section {name} is marked split but has no `for f in ...; do` loop")
    j = loop_end(body, i)
    head, loop, tail = body[:i], body[i : j + 1], body[j + 1 :]
    pats = re.match(r"for f in (.*); do\s*$", loop[0]).group(1).split()
    files = [f for p in pats for f in (sorted(glob.glob(p, root_dir=ROOT)) or [p])]
    out = [(f"{name}:{f}", head + [f"for f in {sh_quote(f)}; do\n"] + loop[1:]) for f in files]
    if any(l.strip() and not l.startswith("#") for l in tail):
        out.append((f"{name}:rest", head + tail))
    return out


def sh_quote(s):
    return "'" + s.replace("'", "'\\''") + "'"


def run(script, args, tmp):
    os.makedirs(tmp, exist_ok=True)
    # check.sh keeps its scratch files in /tmp under fixed names: give each job its own folder for them. Files a
    # test program itself makes (/tmp/brevi_sb_*, which the sandbox checks look for) keep their names.
    text = re.sub(r"/tmp/brevi(?=[_-])(?!_sb_)", tmp + "/brevi", "".join(script)) + "\nexit $fail\n"
    t = time.time()
    with (SCOPED if "tests/sandbox/scoped" in text or uses_scoped(script) else NOLOCK):
        # TMPDIR too: brevi builds a file in TMPDIR/brevi-HASH-OF-ITS-PATH, so two jobs building the same file
        # (tests/bv_libs and their --sandbox runs) would share that folder and clobber each other's build.
        p = subprocess.run(["sh", "-c", text, "check.sh"] + args, cwd=ROOT, capture_output=True, text=True, env=dict(os.environ, TMPDIR=tmp))
    return p.returncode, p.stdout + p.stderr, time.time() - t


# Seconds one job and a whole cold run may take on 4 cores before the runner warns (a cold run took ~25 min on
# 2026-11-09; the slowest job, mcp_http, 261 s).
JOB_BUDGET = 300
RUN_BUDGET = 30 * 60

# Checks that write into tests/sandbox/scoped run one at a time.
SCOPED = threading.Lock()
NOLOCK = contextlib.nullcontext()


def uses_scoped(script):
    m = re.search(r"^for f in '([^']*)'; do", "".join(script), re.M)
    try:
        return m is not None and "sandbox/scoped" in open(os.path.join(ROOT, m.group(1))).read()
    except OSError:
        return False


def main():
    args = sys.argv[1:]
    n = int(os.environ.get("JOBS", os.cpu_count() or 4))
    quiet = "--quiet" in args
    args = [a for a in args if a != "--quiet"]
    if "--jobs" in args:
        k = args.index("--jobs")
        n = int(args[k + 1])
        del args[k : k + 2]
    skip = set()
    if "--changed-since" in args:
        k = args.index("--changed-since")
        skip, why = skipped_sections(args[k + 1])
        del args[k : k + 2]
        if why:
            print(f"check_parallel: every section runs ({why})", file=sys.stderr)
        elif skip:
            print("check_parallel: fast tier, skipping the library sections nothing here changes: "
                  + ", ".join(sorted(skip)) + " (CI and releases run them)", file=sys.stderr)
    with open(CHECK) as fh:
        secs = [s for s in sections(fh.readlines()) if s[0] not in skip]
    only = [x for x in os.environ.get("CHECK_ONLY", "").split(",") if x]  # CHECK_ONLY=polars,jinja2: just those
    if only:
        secs = secs[:1] + [s for s in secs[1:] if s[0] in only]
    leave = [x for x in os.environ.get("CHECK_SKIP", "").split(",") if x]  # CHECK_SKIP=mcp_sdk: run those elsewhere
    if leave:
        secs = secs[:1] + [s for s in secs[1:] if s[0] not in leave]
    pre = secs[0][2]
    # The preamble (BREVI=..., fail=0) goes in front of every job; it must not run checks itself.
    if any(re.match(r"(for|if|while) ", l) for l in pre):
        raise SystemExit("check_parallel: put a `#== NAME` line before the first check in check.sh")
    jobs = [(label, pre + body) for name, split, b in secs[1:] for label, body in jobs_of(name, split, b)]
    os.environ.setdefault("BREVI_CACHE_MB", "6144")  # room for every test program: the cache never evicts mid-run
    os.environ["CHECK_SERVERS"] = "1"
    # No background optimised builds (src/tier.rs): every test program would start one. The "tiered builds"
    # check turns them on for itself.
    os.environ.setdefault("BREVI_TIERED", "0")
    top = tempfile.mkdtemp(prefix="brevi-check-")
    procs = []
    try:
        for cmd, ready in SERVERS:
            log = os.path.join(top, os.path.basename(cmd[1]) + ".log")
            procs.append(subprocess.Popen(cmd, cwd=ROOT, stdout=open(log, "w"), stderr=subprocess.STDOUT))
            for _ in range(40):
                if ready in open(log).read():
                    break
                time.sleep(0.25)
        t0 = time.time()
        # Longest jobs first, by what the last run took (else in file order), so a slow one does not start last.
        hist = os.path.join(os.environ.get("BREVI_CACHE", os.path.expanduser("~/.cache/brevi")), "check-times.txt")
        took = {}
        try:
            for l in open(hist):
                s, label = l.rstrip("\n").split(" ", 1)
                took[label] = float(s)
        except (OSError, ValueError):
            pass
        order = sorted(range(len(jobs)), key=lambda k: -took.get(jobs[k][0], 0))
        res = [None] * len(jobs)
        with cf.ThreadPoolExecutor(n) as ex:
            futs = {ex.submit(run, jobs[k][1], args, os.path.join(top, str(k))): k for k in order}
            nxt = 0
            for fu in cf.as_completed(futs):
                res[futs[fu]] = fu.result()
                while nxt < len(jobs) and res[nxt] is not None:  # print in check.sh's order as soon as we can
                    out = res[nxt][1]
                    if quiet:
                        out = "".join(l for l in out.splitlines(True) if not l.startswith("ok "))
                    sys.stdout.write(out)
                    sys.stdout.flush()
                    nxt += 1
        try:
            os.makedirs(os.path.dirname(hist), exist_ok=True)
            for (label, _), r in zip(jobs, res):
                took[label] = r[2]
            with open(hist, "w") as fh:
                for label, s in took.items():
                    fh.write(f"{s:.1f} {label}\n")
        except OSError:
            pass
        failed = [jobs[k][0] for k, r in enumerate(res) if r[0] != 0]
        slow = sorted(((r[2], jobs[k][0]) for k, r in enumerate(res)), reverse=True)[:5]
        print(f"check_parallel: {len(jobs)} jobs on {n} at once, {time.time() - t0:.0f} s; slowest: "
              + ", ".join(f"{label} {s:.0f} s" for s, label in slow), file=sys.stderr)
        # The time budget (Adam 2026-11-09: keep these tests from getting slow again). Over it is a warning, not a
        # failure: speed up the test, or what it builds, rather than raising the numbers.
        over = [(r[2], jobs[k][0]) for k, r in enumerate(res) if r[2] > JOB_BUDGET]
        if over:
            print(f"check_parallel: over the {JOB_BUDGET} s budget for one job: "
                  + ", ".join(f"{label} {s:.0f} s" for s, label in sorted(over, reverse=True)), file=sys.stderr)
        if time.time() - t0 > RUN_BUDGET:
            print(f"check_parallel: the run took over its {RUN_BUDGET // 60} min budget", file=sys.stderr)
        if failed:
            print("check_parallel: failing jobs: " + ", ".join(failed), file=sys.stderr)
        return 1 if failed else 0
    finally:
        for p in procs:
            p.kill()
        shutil.rmtree(top, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
