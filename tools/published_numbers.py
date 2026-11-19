"""The one canonical table of Brevi's published numbers, and the check that holds every page to it.

usage: python3 tools/published_numbers.py --tag v0.28.1         print docs/NUMBERS.md for a release
       python3 tools/published_numbers.py --write --tag v0.28.1 write it (after a release, with the docs' score update)
       python3 tools/published_numbers.py --check               (CI) docs/NUMBERS.md is what its tag's reports give, and no
                                                       public page quotes a headline figure without its scope

Every figure comes from data, never from a page: the conformance shares from the release tag's own
bench/conformance reports (counts, rounded once with tools/share.py), the speed medians from bench/compare/speed.json.
--check also refuses, in every published text file (README, docs, site, package READMEs, agent skills, the MCP
server's text, the tag's release notes):
- a line quoting the core coverage share without "of the scored suite, with N string-as-code tests excluded";
- a line quoting a typed-build speed median without the dynamic build's median beside it.
"""
import json, os, re, subprocess, sys
from decimal import Decimal, ROUND_HALF_UP

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from share import pct  # noqa: E402

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
OUT = "docs/NUMBERS.md"
SPEED = "bench/compare/speed.json"
REPO = "https://github.com/ets-public/brevi"
# The files a conformance run writes; a tag may differ from the commit its reports measured only in these.
MEASURED_OUTPUTS = ("bench/conformance/", "compat/conformance_floor.json", "compat/conformance_stdlib_floor.json",
                    "compat/conformance_windows_floor.json", "compat/conformance_windows_stdlib_floor.json")


def git(*a, check=True):
    r = subprocess.run(["git", "-C", ROOT, *a], capture_output=True, text=True)
    if check and r.returncode:
        sys.exit(f"git {' '.join(a)}: {r.stderr.strip()}")
    return r


def shallow():
    return git("rev-parse", "--is-shallow-repository", check=False).stdout.strip() == "true"


def have_tag(tag):
    """Fetches the tag when a shallow CI checkout lacks it. Exits when it does not exist at all."""
    if git("rev-parse", "-q", "--verify", f"refs/tags/{tag}^{{commit}}", check=False).returncode:
        git("fetch", "-q", "--depth=1", "origin", f"refs/tags/{tag}:refs/tags/{tag}", check=False)
    r = git("rev-parse", "-q", "--verify", f"refs/tags/{tag}^{{commit}}", check=False)
    if r.returncode:
        sys.exit(f"tag {tag} not found here or on origin: docs/NUMBERS.md describes a published release")
    return r.stdout.strip()


def at_tag(tag, path):
    return git("show", f"refs/tags/{tag}:{path}").stdout


def provenance(tag, tag_commit=None):
    """(measured commit, tag commit, changed files): the commit both conformance reports measured, the commit the tag
    is, and what differs between them. Exits unless both reports measured one commit, it is an ancestor of the tag
    (fetched if the checkout is shallow), and only measured outputs differ: so the published numbers describe exactly
    the released code, and saying "measured at X, tagged at Y" is checked, not asserted."""
    tag_commit = tag_commit or have_tag(tag)
    measured = set()
    for rel in ("report.md", "stdlib/report.md"):
        head = git("show", f"{tag_commit}:bench/conformance/{rel}").stdout
        m = re.search(r"^Brevi \S+ \(commit ([0-9a-f]+)\) against CPython", head, re.M)
        if not m:
            sys.exit(f"bench/conformance/{rel} at {tag}: no clean measured commit in its header")
        measured.add(m.group(1))
    if len(measured) != 1:
        sys.exit(f"{tag}: the core and stdlib reports measured different commits ({', '.join(sorted(measured))})")
    short = measured.pop()
    if git("rev-parse", "-q", "--verify", f"{short}^{{commit}}", check=False).returncode:
        git("fetch", "-q", "--deepen=50", "origin", check=False)
    full = git("rev-parse", "-q", "--verify", f"{short}^{{commit}}", check=False)
    if full.returncode:
        sys.exit(f"{tag}: measured commit {short} not found")
    full = full.stdout.strip()
    if git("merge-base", "--is-ancestor", full, tag_commit, check=False).returncode and shallow():
        # A tag fetched with --depth=1 is a shallow root: its parents are cut off, so ancestry cannot be seen until
        # the tag's own history is deepened (deepening the branch alone leaves that root in place).
        git("fetch", "-q", "--deepen=50", "origin", f"refs/tags/{tag}:refs/tags/{tag}", check=False)
    if git("merge-base", "--is-ancestor", full, tag_commit, check=False).returncode:
        sys.exit(f"{tag}: measured commit {short} is not an ancestor of the tag's commit {tag_commit[:12]}")
    changed = git("diff", "--name-only", full, tag_commit).stdout.split()
    other = [f for f in changed if not f.startswith(MEASURED_OUTPUTS)]
    if other:
        sys.exit(f"{tag}: its commit {tag_commit[:12]} changes more than the reports since the measured commit {short}: "
                 + ", ".join(other[:10]))
    return full, tag_commit, changed


def provenance_line(tag, tag_commit=None):
    """One sentence for release notes and docs/NUMBERS.md."""
    measured, tagged, changed = provenance(tag, tag_commit)
    if measured == tagged:
        return f"The reports measured commit `{tagged[:12]}`, the commit {tag} tags."
    return (f"The reports measured commit `{measured[:12]}`; {tag} tags `{tagged[:12]}`, which adds only the "
            f"conformance reports and floors to it ({len(changed)} files: `git diff --stat {measured[:7]} {tagged[:7]}`), "
            "so the numbers describe exactly the released code.")


def slice_numbers(tag, rel):
    j = json.loads(at_tag(tag, f"bench/conformance/{rel}"))
    n, d = j["totals"]["normal"], j["totals"]["dyn"]
    native = n["matched"] - n["matched_through_cpython"]
    p = j.get("paths", {}).get("normal", {})
    return {"n": n, "native": native, "dyn_native": d["matched"] - d["matched_through_cpython"], "paths": p,
            "cov": pct(native, n["scored"]), "dyn_cov": pct(d["matched"] - d["matched_through_cpython"], d["scored"]),
            "assert": pct(n["matched_asserting"], n["scored"]), "typed": pct(n["matched_typed"], native),
            "head": at_tag(tag, f"bench/conformance/{rel[:-5]}.md").splitlines()[2],
            "printed_assert": re.search(r"\*\*Assertion-bearing coverage ([\d.]+%)", at_tag(tag, f"bench/conformance/{rel[:-5]}.md")).group(1)}


def median(xs):
    xs = sorted(xs)
    k = len(xs) // 2
    return Decimal(str(xs[k])) if len(xs) % 2 else (Decimal(str(xs[k - 1])) + Decimal(str(xs[k]))) / 2


def times(x):
    """A speed-up as the docs print it: two decimals below 1x (0.85x), else one (9.3x), rounded half up once."""
    q = Decimal("0.01") if x < 1 else Decimal("0.1")
    return f"{Decimal(x).quantize(q, ROUND_HALF_UP)}x"


def speed():
    s = json.load(open(os.path.join(ROOT, SPEED)))
    out = {}
    for build in ("typed", "release", "dynamic"):
        xs = [p[build] for p in s["programs"].values()]
        out[build] = {"median": times(median(xs)), "lo": f"{min(xs)}x", "hi": f"{max(xs)}x",
                      "slower": sum(1 for x in xs if x < 1)}
    return s, out


def scope(c):
    """The words every quote of the core headline share must carry."""
    return f"of the scored suite, with {c['n']['by_design']} string-as-code tests excluded"


def render(tag):
    tag_commit = have_tag(tag)
    c, s = slice_numbers(tag, "report.json"), slice_numbers(tag, "stdlib/report.json")
    sp, med = speed()
    wfloor = json.loads(at_tag(tag, "compat/conformance_windows_floor.json"))["matched"]["normal"]
    # The stdlib slice gates a Windows release only when the tag's release.yml runs it with --release-gate (v0.28.2 on).
    wstd = None
    if any("--slice stdlib" in ln and "--release-gate" in ln
           for ln in at_tag(tag, ".github/workflows/release.yml").splitlines()):
        wstd = json.loads(at_tag(tag, "compat/conformance_windows_stdlib_floor.json"))["matched"]["normal"]
    blob = f"{REPO}/blob/{tag}/"
    num = lambda x: f"{x:,}"
    share = lambda a, b: f"{num(a)} of {num(b)} ({pct(a, b)})"
    row = lambda label, f: f"| {label} | {f(c)} | {f(s)} |"
    total_paths = lambda x: sum(x["paths"].values())
    L = [
        "# Brevi in numbers",
        "",
        f"<!-- Written by `python3 tools/published_numbers.py --write --tag {tag}` from the tag's own conformance reports and "
        f"{SPEED}. Never edit by hand: CI (`tools/published_numbers.py --check`) refuses a copy that differs. -->",
        "",
        f"The one table every page, release note and site quotes. Release **{tag}** (commit `{tag_commit[:12]}`). "
        + provenance_line(tag, tag_commit),
        "",
        "## Conformance: CPython 3.13's own tests",
        "",
        f"Native builds of CPython {c['head'].split('against CPython ')[1].split(' ')[0]}'s own tests, Linux, at "
        "`PYTHONHASHSEED=0`. Each scored test must print exactly what CPython prints. Reports: "
        f"[core]({blob}bench/conformance/report.md), [standard library]({blob}bench/conformance/stdlib/report.md).",
        "",
        "| | Core language | Standard library |",
        "| --- | ---: | ---: |",
        row("Tests in the slice", lambda x: num(x["n"]["total"])),
        row("Refused by design (run a string as code), excluded", lambda x: num(x["n"]["by_design"])),
        row("Not scored: output unstable between CPython runs, or excluded by the harness",
            lambda x: num(x["n"]["unstable"] + x["n"]["excluded"])),
        row("**Scored**", lambda x: f"**{num(x['n']['scored'])}**"),
        row("**Match CPython, native build**", lambda x: f"**{num(x['native'])} of {num(x['n']['scored'])}** "
            f"({x['cov']} {scope(x)})"),
        row("Give a different answer", lambda x: num(x["n"]["wrong"])),
        row("Refused at build time (count as not matched)", lambda x: num(x["n"]["refused"])),
        row("Could not be extracted (count as not matched)", lambda x: num(x["n"]["not_extractable"])),
        row("Matched only through the CPython bridge (not counted)", lambda x: num(x["n"]["matched_through_cpython"])),
        row("Matches that print something to compare (assertion-bearing)",
            lambda x: share(x["n"]["matched_asserting"], x["n"]["scored"])),
        row("Matches that print nothing (built and exited 0)", lambda x: num(x["n"]["matched_silent"])),
        row("Matches that took the typed build (typed share)", lambda x: share(x["n"]["matched_typed"], x["native"])),
        row("Forced dynamic build (`BREVI_DYN=1`): match", lambda x: f"{num(x['dyn_native'])} of {num(x['n']['scored'])} "
            f"({x['dyn_cov']} {scope(x)})"),
        "",
        f"**How to quote the headline:** {c['cov']} {scope(c)} ({num(c['native'])} of {num(c['n']['scored'])}). "
        f"The standard library slice: {s['cov']} {scope(s)}. Never quote a coverage share without its scope; "
        "CI refuses a page that does.",
        "",
    ] + [
        f"The {name} report at {tag} prints the assertion-bearing share as {x['printed_assert']}: it rounded twice "
        f"(a fix in tools/share.py rounds once, from the counts). {num(x['n']['matched_asserting'])} of {num(x['n']['scored'])} "
        f"is {x['assert']}, the figure to quote.\n"
        for name, x in (("core", c), ("standard library", s)) if x["printed_assert"] != x["assert"]
    ] + [
        "Which build the conformance programs took (normal build, every program, not just matches): "
        + "; ".join(f"{name} {num(total_paths(x))} programs: typed {share(x['paths'].get('typed', 0), total_paths(x))}, "
                    f"dynamic {share(x['paths'].get('dynamic', 0), total_paths(x))}, "
                    f"pybridge {num(x['paths'].get('pybridge', 0))}, refused {num(x['paths'].get('refused', 0))}"
                    for name, x in (("core", c), ("stdlib", s))) + ".",
        "",
        "## Speed: typed build against dynamic build",
        "",
        f"bench/compare's 12 compute programs, speed-up over CPython {sp['cpython']} (best of 5, one at a time). "
        f"Measured {sp['date']} on Brevi {sp['brevi']} (`{sp['brevi_commit']}`), {sp['machine']}; not yet re-measured "
        f"on {tag}. Data: [{SPEED}](../{SPEED}).",
        "",
        "| | Typed, default build | Typed, `--release` | Dynamic (`BREVI_DYN=1 --release`) |",
        "| --- | ---: | ---: | ---: |",
    ]
    builds = ("typed", "release", "dynamic")
    L.append("| **Median** | " + " | ".join(f"**{med[b]['median']}**" for b in builds) + " |")
    L.append("| Range | " + " | ".join(f"{med[b]['lo']} to {med[b]['hi']}" for b in builds) + " |")
    L.append("| Slower than CPython | " + " | ".join(f"{med[b]['slower']} of {len(sp['programs'])}" for b in builds) + " |")
    L += [
        "",
        f"**How to quote the speed:** typed median {med['typed']['median']}, dynamic median {med['dynamic']['median']}. "
        f"The {med['typed']['median']} figure holds only on the typed path: all 12 of these programs build typed, "
        f"but only {c['typed']} of core and {s['typed']} of standard-library conformance matches took the typed build. "
        "A program whose values change type gets the dynamic build, which runs at about CPython's speed or slower. "
        "CI refuses a page that quotes a typed median without the dynamic one.",
        "",
        "## Windows",
        "",
        (f"The {tag} release gate held the core slice on the shipped `brevi.exe` (at least 90%, no wrong answer; Windows "
         f"floor {num(wfloor)} native matches), on x86-64 and ARM64, and the standard-library slice too (at least 80%, "
         f"no wrong answer; Windows floor {num(wstd)} native matches). Windows limits are in "
         "[KNOWN_ISSUES.md](KNOWN_ISSUES.md#windows-beta)." if wstd is not None else
         f"The {tag} release gate held the core slice on the shipped `brevi.exe` (at least 90%, no wrong answer; Windows "
         f"floor {num(wfloor)} native matches). The standard-library slice was not gated on Windows in {tag}; its limits "
         "are in [KNOWN_ISSUES.md](KNOWN_ISSUES.md#windows-beta)."),
        "",
    ]
    return "\n".join(L)


PUBLIC = re.compile(r"^(README\.md|BREVI\.md|REFERENCE\.md|CHANGELOG\.md|changelog\.d/.+\.md|docs/.+|"
                    r"site/(?!out).+\.(html|md|txt)|packaging/.+\.md|skills/.+|plugins/.+\.(md|mdc|json)|"
                    r"bench/README\.md|bench/[^/]+/README\.md|src/mcp\.rs)$")


FEW = re.compile(r"small (fraction|share)", re.I)


def scan(name, text, c, med):
    """Lines of one published text that quote a headline figure without its scope."""
    problems = []
    core_shares = {c["cov"], c["dyn_cov"]}
    need = scope(c)
    typed = {med["typed"]["median"], med["release"]["median"]}
    dyn = med["dynamic"]["median"]
    for i, line in enumerate(text.splitlines(), 1):
        for share in core_shares:
            for m in re.finditer(r"(?<![\d.])" + re.escape(share[:-1]) + r"\s?%", line):
                if re.search(r"width:\s*$", line[:m.start()]):
                    continue  # a CSS bar's width, not a quoted figure
                if need not in line:
                    problems.append(f"{name}:{i}: quotes {share} without \"{need}\"")
                    break
        for share in core_shares:
            whole = str(round(float(share[:-1])))
            if re.search(r"(?<![\d.])" + whole + r"\s?%(?![\d])", line) and need not in line:
                problems.append(f"{name}:{i}: quotes a rounded {whole}% without \"{need}\"")
                break
        for t in typed:
            if re.search(r"(?<![\d.])" + re.escape(t) + r"(?![\d])", line):
                if dyn not in line:
                    problems.append(f"{name}:{i}: quotes the typed median {t} without the dynamic median {dyn}")
                elif name != OUT and not line.lstrip().startswith("|") and not FEW.search(line) and not (c.get("typed") and c["typed"] in line):
                    problems.append(f"{name}:{i}: quotes the typed median {t} without saying only a small fraction of programs take the typed build")
    return problems


def check():
    text = open(os.path.join(ROOT, OUT), encoding="utf-8").read()
    m = re.search(r"--write --tag (v[\d.]+)", text)
    if not m:
        sys.exit(f"{OUT}: no tag in its header")
    tag = m.group(1)
    problems = []
    if text != render(tag):
        problems.append(f"{OUT} is not what {tag}'s reports and {SPEED} give: run python3 tools/published_numbers.py --write --tag {tag}")
    c = slice_numbers(tag, "report.json")
    _, med = speed()
    for f in git("ls-files").stdout.split("\n"):
        if f and PUBLIC.match(f):
            try:
                body = open(os.path.join(ROOT, f), encoding="utf-8").read()
            except (UnicodeDecodeError, FileNotFoundError):
                continue
            problems += scan(f, body, c, med)
    notes = subprocess.run([sys.executable, os.path.join(ROOT, "tools", "release_notes.py"), tag],
                           capture_output=True, text=True, cwd=ROOT)
    if notes.returncode:
        problems.append(f"tools/release_notes.py {tag} failed: {notes.stderr.strip()[-300:]}")
    else:
        problems += scan(f"release notes for {tag}", notes.stdout, c, med)
    if problems:
        sys.exit("Published numbers out of line with docs/NUMBERS.md:\n  " + "\n  ".join(problems))
    print(f"{OUT} matches {tag}; every quoted headline figure carries its scope")


def main():
    a = sys.argv[1:]
    if a == ["--check"]:
        return check()
    if "--tag" not in a:
        sys.exit(__doc__)
    tag = a[a.index("--tag") + 1]
    out = render(tag)
    if "--write" in a:
        open(os.path.join(ROOT, OUT), "w", encoding="utf-8").write(out)
        print(f"wrote {OUT} for {tag}")
    else:
        print(out, end="")


if __name__ == "__main__":
    main()
