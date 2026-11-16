"""Prints the GitHub release notes for a tag: the conformance numbers in full, then its section of CHANGELOG.md, then the install note.
usage: python3 tools/release_notes.py v0.18.0
       python3 tools/release_notes.py --check v0.18.0   (release.yml preflight: the conformance reports measured this tree and hold every figure the notes quote)"""
import json, os, re, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from share import pct  # noqa: E402  (every published share is printed one way)

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


def read_report(tag, rel):
    """A published report as the tag holds it (so rewriting an old release's notes never quotes a later main's
    numbers), or the working tree's copy before the tag exists (release.yml writes the notes from the tag's own
    commit, so both are the same file). None when neither has it."""
    path = "bench/conformance/" + rel
    r = subprocess.run(["git", "-C", ROOT, "show", f"refs/tags/{tag}:{path}"], capture_output=True, text=True)
    if r.returncode == 0:
        return r.stdout
    exists = subprocess.run(["git", "-C", ROOT, "rev-parse", "-q", "--verify", f"refs/tags/{tag}"], capture_output=True)
    if exists.returncode == 0:
        return None  # the tag exists and has no such report: quote nothing rather than another tree's numbers
    f = os.path.join(ROOT, path)
    return open(f).read() if os.path.exists(f) else None


FIGURES = {
    "cov": r"\*\*Coverage ([\d.]+%)\*\* \((\d+) of (\d+) tests",
    "wrong": r"\*\*Wrong: (\d+)\*\*",
    "design": r"\*\*Refused by design: (\d+)\*\*",
    "asserting": r"\*\*Assertion-bearing coverage ([\d.]+%)\*\* \((\d+) of (\d+):[^;]*; (\d+) matches print nothing",
    "typed": r"\*\*Typed-path share ([\d.]+%)\*\* \((\d+) of (\d+) native matches",
    "who": r"^Brevi (\S+)(?: \(commit [0-9a-f]+(?:-dirty)?\))? against CPython (\S+)[^,]*, (\d{4}-\d\d-\d\d)",
}


DYN = r"Forced dynamic build \(`BREVI_DYN=1`\): coverage ([\d.]+%)"


def check_figures(md, js, rel):
    """What is wrong with one report's published shares: each must be pct() of the counts beside it, and those
    counts must be report.json's. Empty when the report holds. --check runs it, so a report printed with another
    rounding (or by hand) never reaches release notes."""
    m = {k: re.search(v, md, re.M) for k, v in FIGURES.items()}
    m["dyn"] = re.search(DYN, md)
    problems = [f"bench/conformance/{rel}: no {k} figure" for k, x in m.items() if not x]
    t = json.loads(js).get("totals", {}) if js else {}
    n, d = t.get("normal"), t.get("dyn")
    if not (n and d):
        return problems + [f"bench/conformance/{rel}: report.json has no normal and dyn totals"]
    native = lambda x: x["matched"] - x["matched_through_cpython"]
    want = {"cov": (native(n), n["scored"]), "asserting": (n["matched_asserting"], n["scored"]),
            "typed": (n["matched_typed"], native(n))}
    for k, (a, b) in want.items():
        if not m[k]:
            continue
        shown, sa, sb = m[k].group(1), int(m[k].group(2)), int(m[k].group(3))
        if (sa, sb) != (a, b):
            problems.append(f"bench/conformance/{rel}: {k} says {sa} of {sb}, report.json says {a} of {b}")
        if shown != pct(sa, sb):
            problems.append(f"bench/conformance/{rel}: {k} prints {shown} for {sa} of {sb}, which is {pct(sa, sb)}")
    if m["dyn"] and m["dyn"].group(1) != pct(native(d), d["scored"]):
        problems.append(f"bench/conformance/{rel}: forced dynamic coverage prints {m['dyn'].group(1)}, "
                        f"report.json's counts give {pct(native(d), d['scored'])}")
    return problems


def figures(r, rel):
    """Every number the release notes lead with, from one report. A report missing any of them stops the notes:
    they quote the full set or nothing, never the headline alone."""
    got = {k: re.search(v, r, re.M) for k, v in FIGURES.items()}
    missing = [k for k, m in got.items() if not m]
    if missing:
        sys.exit(f"bench/conformance/{rel}: no {', '.join(missing)} figure; the release notes quote every figure or none")
    return got


def main():
    if sys.argv[1] == "--check":
        check_reports(sys.argv[2])
        problems = []
        for rel in ("report.md", "stdlib/report.md"):
            base = os.path.join(ROOT, "bench", "conformance", rel)
            problems += check_figures(open(base).read(), open(base[:-3] + ".json").read(), rel)
        if problems:
            sys.exit("The conformance reports' figures do not hold:\n  " + "\n  ".join(problems))
        print("every published share is pct() of its counts")
        return
    tag = sys.argv[1]
    import changelog
    text = changelog.changelog()  # CHANGELOG.md with any changelog.d/ fragments in its Unreleased section
    # A section starts at "## v0.18.0 (date)" and runs to the next "## ".
    m = re.search(r"^## " + re.escape(tag) + r"\b[^\n]*\n(.*?)(?=^## |\Z)", text, re.S | re.M)
    body = m.group(1).strip() if m else "See CHANGELOG.md."

    # The notes lead with the CPython conformance numbers in full: not only the headline share, but what it leaves out
    # (tests refused by design), how strong its matches are (the share that printed something to compare) and how many
    # took the typed build. Core is CPython's own language tests; stdlib is its tests of the modules agents use most
    # (tools/conformance.py --slice stdlib).
    core, std = read_report(tag, "report.md"), read_report(tag, "stdlib/report.md")
    if core and std:
        c, s = figures(core, "report.md"), figures(std, "stdlib/report.md")
        blob = f"https://github.com/ets-public/brevi/blob/{tag}/bench/conformance/"
        row = lambda label, f: print(f"| {label} | {f(c)} | {f(s)} |")
        print(f"## Conformance, in full\n")
        print(f"Native builds of CPython {c['who'].group(2)}'s own tests, measured on Brevi {c['who'].group(1)} "
              f"({c['who'].group(3)}). Each test must print exactly what CPython prints.\n")
        print(f"| | [Core language]({blob}report.md) | [Standard library]({blob}stdlib/report.md) |")
        print("| --- | ---: | ---: |")
        # Shares are worked out here from the report's counts with pct(): a report written before tools/share.py
        # rounded twice (945 of 1072 read 88.1%, not 88.2%).
        share = lambda m: f"{m.group(2)} of {m.group(3)} ({pct(m.group(2), m.group(3))})"
        row("Match CPython", lambda f: share(f["cov"]))
        row("Give a different answer", lambda f: f["wrong"].group(1))
        row("Refused by design, left out of the count", lambda f: f["design"].group(1))
        row("Matches that print something to compare", lambda f: share(f["asserting"]))
        row("Matches that print nothing (built and exited 0)", lambda f: f["asserting"].group(4))
        row("Matches that took the typed build", lambda f: share(f["typed"]))
        print()
        print("Refused by design means the test runs a string as code (compile, exec or eval), which a sandboxed native "
              "binary refuses on purpose. A match that prints nothing only shows the program built and exited 0. The "
              "standard library tests cover itertools, collections, re, textwrap, functools, string, graphlib, colorsys, "
              "deque, dataclasses and enum. Check your own program the same way with `brevi verify`.")
        print()
    elif core or std:
        sys.exit("only one of the two conformance reports exists for " + tag + "; the release notes quote both or neither")

    # Which path the conformance programs took (the report's "paths", normal build): how much ran on the typed fast
    # path, how much fell back to the dynamic build, and how much Brevi refused (python3 would have run it, or
    # --strict / --native refuse it). One line per slice.
    paths_lines = []
    for name, rel in (("core", "report.json"), ("stdlib", "stdlib/report.json")):
        raw = read_report(tag, rel)
        if not raw:
            continue
        p = json.loads(raw).get("paths", {}).get("normal", {})
        total = sum(p.values())
        if not total:
            continue
        part = lambda k: f"{p.get(k, 0)} ({pct(p.get(k, 0), total)})"
        paths_lines.append(f"{name} {total} programs: typed {part('typed')}, pybridge {part('pybridge')}, "
                           f"dynamic fallback {part('dynamic')}, refused {part('refused')}")
    if paths_lines:
        print("**Paths taken:** " + "; ".join(paths_lines) + ". Only typed and pybridge pass `--strict`, "
              "which fails closed on any fallback; every build reports its path ([support matrix](https://github.com/ets-public/brevi/blob/"
              + tag + "/docs/SUPPORT_MATRIX.md)).")
        print()

    if core and std:
        print("## Changes\n")
    print(body)
    print()
    # Outside contributors, thanked by handle in the section's entries ("thanks @name"), are listed by name too.
    thanked = sorted(set(re.findall(r"thanks (@[A-Za-z0-9-]+)", body)), key=str.lower)
    if thanked:
        print("**Contributors:** thanks to " + ", ".join(thanked) + " for their work in this release.")
        print()
    print("Prebuilt brevi compiler for Linux, macOS and Windows. brevi still needs rustc on the PATH to build programs. "
          "On Linux and macOS, `sh install.sh` installs rustc if it is missing and picks the right download; on Windows, "
          "install Rust with rustup and unzip the .zip. Details: https://brevi-lang.org/docs/install")


if __name__ == "__main__":
    main()
