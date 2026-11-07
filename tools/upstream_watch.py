#!/usr/bin/env python3
"""Watches Rust, Python, crate and MCP specification releases and opens a GitHub issue for each new one Brevi has not reviewed.

Each issue carries a review checklist (and, for Rust, the release's stabilized APIs) so new language
features and stdlib idioms get adopted instead of Brevi falling behind. Issues are deduplicated by title,
so this is safe to run as often as you like. Needs GH_TOKEN and GITHUB_REPOSITORY (set in Actions).

usage: upstream_watch.py [--dry-run]
"""
import json, os, re, subprocess, sys, urllib.error, urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = os.environ.get("GITHUB_REPOSITORY", "ets-public/brevi")
DRY = "--dry-run" in sys.argv
LABEL = "upstream"


def get(url, raw=False):
    h = {"User-Agent": "brevi-upstream-watch", "Accept": "application/vnd.github+json"}
    if "api.github.com" in url and os.environ.get("GH_TOKEN"):
        h["Authorization"] = "Bearer " + os.environ["GH_TOKEN"]
    with urllib.request.urlopen(urllib.request.Request(url, headers=h), timeout=30) as r:
        body = r.read().decode()
    return body if raw else json.loads(body)


def gh(*args):
    return subprocess.run(["gh", *args, "--repo", REPO], capture_output=True, text=True, check=True).stdout


def issue_exists(title):
    out = gh("issue", "list", "--state", "all", "--label", LABEL, "--search", f'"{title}" in:title', "--json", "title", "--limit", "50")
    return any(i["title"] == title for i in json.loads(out))


def open_issue(title, body):
    if DRY:
        print(f"=== {title}\n{body}\n")
        return
    if issue_exists(title):
        print(f"already tracked: {title}")
        return
    url = gh("issue", "create", "--title", title, "--body", body, "--label", LABEL).strip()
    print(f"opened: {title} {url}")
    # Issues opened with the Actions token do not trigger other workflows, so start auto-fix directly.
    try:
        gh("workflow", "run", "auto-fix.yml", "-f", f"issue={url.rsplit('/', 1)[-1]}")
    except subprocess.CalledProcessError as e:
        print(f"could not start auto-fix: {e.stderr}")


def part(section, heading):
    """The bullet list under one heading of a RELEASES.md version section, with links stripped."""
    m = re.search(rf"^{heading}\n-+\n(.*?)(?=^<a id=|^[A-Z][^\n]*\n-{{3,}}|\Z)", section, re.M | re.S)
    if not m:
        return "(none listed)"
    t = re.sub(r"\]\((?:[^()\s]|\([^)\s]*\))*\)", "", m.group(1).strip())
    return re.sub(r"(^\s*[-*] |\s)\[", r"\1", t, flags=re.M)


def rust():
    tag = get("https://api.github.com/repos/rust-lang/rust/releases/latest")["tag_name"]
    minor = ".".join(tag.split(".")[:2])
    notes = get("https://raw.githubusercontent.com/rust-lang/rust/master/RELEASES.md", raw=True)
    m = re.search(rf"^Version {re.escape(minor)}\.0 \([^\n]*\n(.*?)(?=^Version \d)", notes, re.M | re.S)
    section = m.group(1) if m else ""
    lang_list, api_list = part(section, "Language"), part(section, "Stabilized APIs")
    tag = minor
    body = f"""Rust {tag} is out: https://blog.rust-lang.org/releases/ and https://github.com/rust-lang/rust/blob/master/RELEASES.md

Brevi compiles to Rust, so every stable release needs a review. Tick each item or note why it does not apply.

- [ ] The Toolchains workflow is green on this stable (it runs daily; old solutions must all still pass)
- [ ] `python3 tools/compat.py lints` shows no new deprecation or future-incompatibility warnings in generated code
- [ ] New or reworded rustc errors are still mapped to short Brevi messages (`to_brevi_words`, `fix_for` in `src/main.rs`; `tests/errors`)
- [ ] Stabilized APIs below: adopt any that make the prelude (`src/prelude.rs`) or generated code shorter, faster or more correct
- [ ] Language changes below: adopt any that simplify `src/emit.rs` output or let Brevi accept syntax models now write
- [ ] If a new edition is stable, plan moving generated code to it (`--edition` in `src/main.rs`)

### Language
{lang_list[:6000]}

### Stabilized APIs
{api_list[:12000]}
"""
    open_issue(f"Review Rust {tag} for Brevi", body)


def python():
    cycles = get("https://endoflife.date/api/python.json")
    newest = max(cycles, key=lambda c: tuple(map(int, c["cycle"].split("."))))
    v = newest["cycle"]
    body = f"""Python {v} is out (latest {newest['latest']}): https://docs.python.org/{v}/whatsnew/{v}.html

Brevi borrows Python's method names, regex syntax and idioms so that models' Python habits just work, and
Python is the benchmark baseline. Tick each item or note why it does not apply.

- [ ] The Toolchains workflow is green on Python {v} (bench scripts and all saved Python solutions still pass)
- [ ] New syntax or stdlib idioms that models will start writing: accept them in Brevi where cheap (`python_method` in `src/parser.rs`, prelude helpers in `src/prelude.rs`), with a test in `tests/run`
- [ ] Regex changes in `re` that Brevi's Python-syntax regexes should follow
- [ ] Benchmark baseline uses Python {v} (rerun `bench/` for the weekly comparison)
- [ ] Python interop (`py fn`) still works with this version
"""
    open_issue(f"Review Python {v} for Brevi", body)


def crates():
    """Crates pinned in Brevi programs with `use NAME "VERSION"`: open an issue when a newer major/minor exists."""
    pins = {}
    for f in list(ROOT.glob("tests/**/*.bv")) + list(ROOT.glob("examples/**/*.bv")):
        for m in re.finditer(r'^\s*use\s+([A-Za-z0-9_-]+)\s+"([0-9][^"]*)"', f.read_text(), re.M):
            pins.setdefault(m.group(1), set()).add(m.group(2))
    for name, vers in sorted(pins.items()):
        try:
            latest = get(f"https://crates.io/api/v1/crates/{name}")["crate"]["max_stable_version"]
        except Exception as e:
            print(f"crates.io lookup failed for {name}: {e}")
            continue
        key = lambda s: [int(x) for x in re.findall(r"\d+", s)[:2]]
        if all(key(latest) > key(v) for v in vers):
            body = f"""Crate `{name}` {latest} is out; Brevi's tests and examples pin {', '.join(sorted(vers))}.
https://crates.io/crates/{name}

- [ ] Bump the `use {name} "..."` lines and check `tests/check.sh` still passes
- [ ] Note any API change Brevi programs need in BREVI.md
"""
            open_issue(f"Update crate {name} to {latest}", body)


def mcp_spec():
    """The MCP specification Brevi's server speaks: open an issue when a newer protocol version is published."""
    req = urllib.request.Request("https://modelcontextprotocol.io/specification/latest", method="HEAD", headers={"User-Agent": "brevi-upstream-watch"})

    class Stop(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *a):
            return None
    try:
        urllib.request.build_opener(Stop).open(req, timeout=30)
        raise RuntimeError("no redirect from /specification/latest")
    except urllib.error.HTTPError as e:
        loc = e.headers.get("Location", "")
    latest = re.search(r"\d{4}-\d{2}-\d{2}", loc).group(0)
    # the newest version the server speaks: handshake ones in VERSIONS, 2026-07-28 on in MODERN
    src = (ROOT / "src/mcp.rs").read_text()
    lists = re.findall(r'const (?:VERSIONS|MODERN): \[&str; \d+\] = \[([^\]]*)\]', src)
    ours = max(re.findall(r'"(\d{4}-\d{2}-\d{2})"', " ".join(lists)))
    if latest > ours:
        body = f"""MCP specification {latest} is published: https://modelcontextprotocol.io/specification/{latest}
(changelog: https://modelcontextprotocol.io/specification/{latest}/changelog). `brevi mcp` speaks up to {ours}.

- [ ] Read the changelog for changes to initialize, tools/list, tools/call, errors and cancellation
- [ ] Support what applies in `src/mcp.rs` and add `{latest}` first in `MODERN` (or `VERSIONS`, for a version with the initialize handshake)
- [ ] `python3 tests/mcp_test.py` passes, and a real client on the new version connects (official MCP Python SDK)
"""
        open_issue(f"Review MCP specification {latest} for Brevi", body)
    else:
        print(f"MCP specification: up to date ({ours})")


if __name__ == "__main__":
    failed = False
    for f in (rust, python, crates, mcp_spec):
        try:
            f()
        except Exception as e:
            print(f"{f.__name__} check failed: {e}", file=sys.stderr)
            failed = True
    sys.exit(1 if failed else 0)
