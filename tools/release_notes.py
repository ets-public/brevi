"""Prints the GitHub release notes for a tag: its section of CHANGELOG.md plus the install note.
usage: python3 tools/release_notes.py v0.18.0
       python3 tools/release_notes.py --check v0.18.0   (release.yml preflight: the conformance reports measured this tree)"""
import os, re, subprocess, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
# The files the conformance run itself writes (main commits them after measuring), so they may differ between the
# measured commit and the release.
MEASURED_OUTPUTS = ("bench/conformance", "compat/conformance_floor.json", "compat/conformance_stdlib_floor.json",
                    "compat/conformance_windows_floor.json")


def check_reports(tag):
    """Fails unless both published conformance reports were measured by this release's version on this release's
    tree: same Brevi version as the tag, and no file changed between the measured commit and HEAD except the
    reports and floors. release.yml runs it in preflight, so the release notes never quote another tree's numbers."""
    problems = []
    for rel in ("report.md", os.path.join("stdlib", "report.md")):
        r = open(os.path.join(ROOT, "bench", "conformance", rel)).read()
        m = re.search(r"^Brevi (\S+) \(commit ([0-9a-f]+)(-dirty)?\) against CPython", r, re.M)
        if not m:
            problems.append(f"bench/conformance/{rel}: no measured commit in its header (made before the report named it)")
            continue
        if m.group(1) != tag.lstrip("v"):
            problems.append(f"bench/conformance/{rel}: measured Brevi {m.group(1)}, not {tag.lstrip('v')}")
        if m.group(3):
            problems.append(f"bench/conformance/{rel}: measured a tree with uncommitted changes ({m.group(2)}-dirty)")
        d = subprocess.run(["git", "-C", ROOT, "diff", "--name-only", m.group(2), "HEAD", "--", "."]
                           + [":!" + x for x in MEASURED_OUTPUTS], capture_output=True, text=True)
        if d.returncode:
            problems.append(f"bench/conformance/{rel}: measured commit {m.group(2)} not in this checkout ({d.stderr.strip()})")
        elif d.stdout.strip():
            problems.append(f"bench/conformance/{rel}: measured {m.group(2)}, and since then these changed: "
                            + ", ".join(d.stdout.split()[:10]))
    if problems:
        sys.exit("The conformance reports do not describe this release. Let conformance-main.yml measure the release "
                 "commit and commit its reports, then release from that commit:\n  " + "\n  ".join(problems))
    print(f"conformance reports measured {tag} on this tree")


if sys.argv[1] == "--check":
    check_reports(sys.argv[2])
    sys.exit(0)
tag = sys.argv[1]
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import changelog
text = changelog.changelog()  # CHANGELOG.md with any changelog.d/ fragments in its Unreleased section
# A section starts at "## v0.18.0 (date)" and runs to the next "## ".
m = re.search(r"^## " + re.escape(tag) + r"\b[^\n]*\n(.*?)(?=^## |\Z)", text, re.S | re.M)
body = m.group(1).strip() if m else "See CHANGELOG.md."
print(body)
print()
# Outside contributors, thanked by handle in the section's entries ("thanks @name"), are listed by name too.
thanked = sorted(set(re.findall(r"thanks (@[A-Za-z0-9-]+)", body)), key=str.lower)
if thanked:
    print("**Contributors:** thanks to " + ", ".join(thanked) + " for their work in this release.")
    print()
def scope(r, *more):
    """What a coverage figure leaves out and how strong its matches are, read from a report: the tests refused by
    design (not in the denominator) and the assertion-bearing share (matches that printed something to compare)."""
    design = re.search(r"\*\*Refused by design: (\d+)\*\*", r)
    asserting = re.search(r"\*\*Assertion-bearing coverage ([\d.]+%)\*\* \((\d+) of", r)
    parts = []
    if design:
        parts.append(f"{design.group(1)} tests that run a string as code are refused by design and left out")
    if asserting:
        parts.append(f"counting only matches that print something to compare, {asserting.group(1)}")
    parts += more
    return (" (" + "; ".join(parts) + ")") if parts else ""


# The latest CPython conformance run (tools/conformance.py writes the report): how often a native build of
# CPython's own tests gives a different answer, published with every release.
report = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "bench", "conformance", "report.md")
if os.path.exists(report):
    r = open(report).read()
    who = re.search(r"^Brevi (\S+)(?: \(commit [0-9a-f]+(?:-dirty)?\))? against CPython (\S+)[^,]*, (\d{4}-\d\d-\d\d)", r, re.M)
    cov = re.search(r"\*\*Coverage ([\d.]+%)\*\* \((\d+) of (\d+) tests", r)
    wrong = re.search(r"\*\*Wrong: (\d+)\*\*", r)
    if who and cov and wrong:
        print(f"**CPython conformance:** {cov.group(2)} of {cov.group(3)} scored tests from CPython {who.group(2)}'s own suite ({cov.group(1)}) "
              f"print exactly what CPython prints from a native build, and {wrong.group(1)} give a different answer"
              + scope(r, f"Brevi {who.group(1)}, {who.group(3)}, [report](https://github.com/ets-public/brevi/blob/{tag}/bench/conformance/report.md)")
              + ". "
              "Check your own program the same way with `brevi verify`.")
        print()
# The second published number: the stdlib modules agents use most (tools/conformance.py --slice stdlib).
report = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "bench", "conformance", "stdlib", "report.md")
if os.path.exists(report):
    r = open(report).read()
    cov = re.search(r"\*\*Coverage ([\d.]+%)\*\* \((\d+) of (\d+) tests", r)
    wrong = re.search(r"\*\*Wrong: (\d+)\*\*", r)
    if cov and wrong:
        print(f"**Standard library:** {cov.group(2)} of {cov.group(3)} tests from CPython's tests of the modules agents use most "
              f"(itertools, collections, re, textwrap, functools, string, graphlib, colorsys, deque, dataclasses, enum; {cov.group(1)}) match, and {wrong.group(1)} give a different answer"
              + scope(r, f"[report](https://github.com/ets-public/brevi/blob/{tag}/bench/conformance/stdlib/report.md)") + ".")
        print()
# Which path the conformance programs took (the report's "paths", normal build): how much ran on the typed fast
# path, how much fell back to the dynamic build, and how much Brevi refused (python3 would have run it, or
# --strict / --native refuse it). One line per slice.
paths_lines = []
for name, rel in (("core", "report.json"), ("stdlib", os.path.join("stdlib", "report.json"))):
    f = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "bench", "conformance", rel)
    if not os.path.exists(f):
        continue
    import json
    p = json.load(open(f)).get("paths", {}).get("normal", {})
    total = sum(p.values())
    if not total:
        continue
    pct = lambda k: f"{p.get(k, 0)} ({100 * p.get(k, 0) / total:.1f}%)"
    paths_lines.append(f"{name} {total} programs: typed {pct('typed')}, pybridge {pct('pybridge')}, "
                       f"dynamic fallback {pct('dynamic')}, refused {pct('refused')}")
if paths_lines:
    print("**Paths taken:** " + "; ".join(paths_lines) + ". Only typed and pybridge pass `--strict`, "
          "which fails closed on any fallback; every build reports its path ([support matrix](https://github.com/ets-public/brevi/blob/"
          + tag + "/docs/SUPPORT_MATRIX.md)).")
    print()
print("Prebuilt brevi compiler for Linux, macOS and Windows. brevi still needs rustc on the PATH to build programs. "
      "On Linux and macOS, `sh install.sh` installs rustc if it is missing and picks the right download; on Windows, "
      "install Rust with rustup and unzip the .zip. Details: https://brevi-lang.org/docs/install")
