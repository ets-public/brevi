#!/usr/bin/env python3
"""Checks whether new releases of the Python libraries Brevi mirrors natively change what Brevi must print.

tools/upstream_pins.json lists each library at the version Brevi's tests/*/NAME.expected files were made with.
For every group with a newer release on PyPI, this installs the pinned versions and the newest ones into two
venvs, runs the group's test programs in CPython with each, and compares the two outputs on the same machine
(so CPU kernels and Python version cancel out). Any test whose output changes is drift Brevi's copy must follow.
It opens one `upstream` issue per new release set (deduplicated by title) with the diffs and a checklist, which
auto-fix.yml then works on. No Brevi build is needed, so it costs a few minutes of Linux time.

usage: parity_drift.py [--dry-run] [--group NAME] [--force]   (--force runs groups that have no new release)
"""
import difflib, glob, json, os, re, subprocess, sys, tempfile, time, urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import upstream_watch as uw  # noqa: E402  (shares get/open_issue and the --dry-run flag)

ROOT = Path(__file__).resolve().parent.parent
CFG = json.loads((ROOT / "tools/upstream_pins.json").read_text())["groups"]
ARGS = sys.argv[1:]
ONLY = ARGS[ARGS.index("--group") + 1] if "--group" in ARGS else None
FORCE = "--force" in ARGS


def vkey(v):
    return [int(x) for x in re.findall(r"\d+", v)]


def latest(pkg):
    """Newest stable release on PyPI (pre-releases skipped)."""
    rel = uw.get(f"https://pypi.org/pypi/{pkg}/json")["releases"]
    ok = [v for v, files in rel.items() if files and re.fullmatch(r"[\d.]+", v) and not all(f.get("yanked") for f in files)]
    return max(ok, key=vkey)


def venv(where, reqs):
    subprocess.run([sys.executable, "-m", "venv", where], check=True)
    py = os.path.join(where, "bin", "python")
    subprocess.run([py, "-m", "pip", "install", "--quiet", "--disable-pip-version-check", *reqs], check=True)
    out = subprocess.run([py, "-m", "pip", "list", "--format", "json"], capture_output=True, text=True, check=True).stdout
    return py, {p["name"].lower(): p["version"] for p in json.loads(out)}


def run_tests(py, g, files):
    env = {k: v for k, v in os.environ.items() if k not in g.get("unset_env", [])}
    srv = None
    if g.get("server"):
        script, port, ready = g["server"]
        log = tempfile.NamedTemporaryFile("w+", delete=False)
        srv = subprocess.Popen([py, script, port], stdout=log, stderr=subprocess.STDOUT, cwd=ROOT)
        for _ in range(40):
            if ready in Path(log.name).read_text():
                break
            time.sleep(0.25)
    out = {}
    try:
        for f in files:
            try:
                p = subprocess.run([py, f], cwd=ROOT, env=env, capture_output=True, text=True, timeout=300)
                out[f] = p.stdout + (p.stderr if g.get("stderr") else "")
            except subprocess.TimeoutExpired:
                out[f] = "<timed out after 300 s>"
    finally:
        if srv:
            srv.kill()
    return out


def check(name, g):
    watch = g["watch"]
    new = {p: latest(p) for p in watch}
    newer = {p: v for p, v in new.items() if vkey(v) > vkey(watch[p])}
    if not newer and not FORCE:
        print(f"{name}: up to date ({', '.join(f'{p} {v}' for p, v in watch.items())})")
        return
    label = ", ".join(f"{p} {v}" for p, v in (newer or new).items())
    files = sorted(f for pat in g["tests"] for f in glob.glob(str(ROOT / pat)))
    files = [os.path.relpath(f, ROOT) for f in files]
    if not files:
        body = f"""New release: {label} (Brevi was checked against {', '.join(f'{p} {v}' for p, v in watch.items())}).

{g.get('note', '')}

- [ ] Read the release notes for protocol or behaviour changes Brevi must follow
- [ ] Verify Brevi against the new version and bump `tools/upstream_pins.json`
"""
        uw.open_issue(f"Parity: review {label}", body)
        return
    with tempfile.TemporaryDirectory() as tmp:
        pins = [f"{p}=={v}" for p, v in {**watch, **g.get("also", {})}.items()]
        old_py, _ = venv(os.path.join(tmp, "old"), pins)
        new_py, got = venv(os.path.join(tmp, "new"), [f"{p}=={v}" for p, v in new.items()])
        before, after = run_tests(old_py, g, files), run_tests(new_py, g, files)
    changed = [f for f in files if before[f] != after[f]]
    def expected(f):
        e = (ROOT / f).with_suffix(".expected")
        return e.read_text().rstrip("\n") if e.exists() else None
    stale = [f for f in files if expected(f) is not None and before[f].rstrip("\n") != expected(f)]
    installed = ", ".join(f"{p} {got.get(p.lower(), '?')}" for p in [*watch, *g.get("also", {})])
    print(f"{name}: {label}: {len(changed)} of {len(files)} tests print something different")
    diffs = []
    for f in changed[:12]:
        d = "\n".join(difflib.unified_diff(before[f].splitlines(), after[f].splitlines(), f"{f} (pinned)", f"{f} (new)", lineterm="", n=1))
        diffs.append(f"<details><summary>{f}</summary>\n\n```diff\n{d[:3000]}\n```\n</details>")
    verdict = (f"**{len(changed)} of {len(files)} test programs print something different** under the new release, so Brevi's own copy must change to match."
               if changed else f"All {len(files)} test programs print the same under the new release: only the pin needs bumping.")
    body = f"""New upstream release: {label}. Brevi mirrors this library natively and must print exactly what it prints.
Installed for the check: {installed}.

{verdict}

{chr(10).join(diffs)}
{'(first 12 shown)' if len(changed) > 12 else ''}
{('Pinned-version runs that no longer match the committed .expected (the runner differs from where they were made; usually CPU kernels): ' + ', '.join(stale)) if stale else ''}

- [ ] Update Brevi's copy so each changed test prints what the new release prints (`sh tests/check.sh` must pass)
- [ ] With the new version installed, regenerate the expected files: `sh tests/check.sh --update`, then review `git diff tests/`
- [ ] Bump the versions in `tools/upstream_pins.json` and in the comment above this group in `tests/check.sh`
- [ ] Note the new supported version in a changelog.d/ fragment and LIBRARIES.md
"""
    uw.open_issue(f"Parity: {label}", body)


if __name__ == "__main__":
    failed = False
    for name, g in CFG.items():
        if ONLY and name != ONLY:
            continue
        try:
            check(name, g)
        except Exception as e:
            print(f"{name}: check failed: {e}", file=sys.stderr)
            failed = True
    sys.exit(1 if failed else 0)
