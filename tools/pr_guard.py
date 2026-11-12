#!/usr/bin/env python3
"""Fails a pull request that breaks one of the merge-collision rules a prompt used to carry.

1. New entries never go into CHANGELOG.md: the release PR folds changelog.d/ into it
   (tools/changelog.py --release). A branch that changes lines in its "## Unreleased" section without
   bumping the version in Cargo.toml is refused: add a fragment in changelog.d/ instead. Fixing a released
   section stays allowed (changelog.d/README.md).
2. The published conformance scores (README.md and the docs below) are owned by the docs thread, which
   updates them from main's reports; feature PRs that each edited them conflicted with one another. A branch
   whose changed lines in those files change a score number (a percentage, "N of M" or "N/M") is refused
   unless the PR title starts with "[scores]" (the docs thread's PRs) or it is a release PR.
   Rewording a line that keeps its numbers passes.

usage: pr_guard.py BASE [--title TITLE]   (CI passes the PR title through the PR_TITLE env var)
"""
import argparse, os, re, subprocess, sys
from collections import Counter

SCORE_FILES = ["README.md", "site/docs/compatibility.md", "docs/METRICS.md", "docs/ROADMAP.md",
               "site/docs/roadmap.md", "docs/KNOWN_ISSUES.md", "docs/VERIFY_BY_DEFAULT.md"]
SCORE = re.compile(r"\d+(?:\.\d+)?%|\b\d+ of \d+\b|\b\d+/\d+\b")


def git(*a):
    return subprocess.run(["git", *a], capture_output=True, text=True, check=True).stdout


def version(rev):
    r = subprocess.run(["git", "show", f"{rev}:Cargo.toml"], capture_output=True, text=True)
    m = re.search(r'(?m)^version\s*=\s*"([^"]+)"', r.stdout)
    return m.group(1) if m else None


def touches_unreleased(base):
    """True when the diff of CHANGELOG.md changes a line between "## Unreleased" and the next heading."""
    lines = open("CHANGELOG.md", encoding="utf-8").read().splitlines()
    start = next((i + 1 for i, l in enumerate(lines) if l.strip().lower() == "## unreleased"), None)
    if start is None:
        return False
    end = next((i + 1 for i, l in enumerate(lines) if i + 1 > start and l.startswith("## ")), len(lines) + 1)
    for h in re.finditer(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@", git("diff", "-U0", base, "--", "CHANGELOG.md"), re.M):
        first, n = int(h.group(1)), int(h.group(2) or 1)
        lo, hi = (first, first + n - 1) if n else (first, first + 1)  # a pure deletion sits after line `first`
        if lo < end and hi >= start:
            return True
    return False


def changed_scores(base, path):
    old, new = Counter(), Counter()
    for line in git("diff", "-U0", base, "--", path).splitlines():
        if line.startswith("-") and not line.startswith("---"):
            old.update(SCORE.findall(line))
        elif line.startswith("+") and not line.startswith("+++"):
            new.update(SCORE.findall(line))
    return sorted((new - old).elements()), sorted((old - new).elements())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("base")
    ap.add_argument("--title", default=os.environ.get("PR_TITLE", ""))
    a = ap.parse_args()
    changed = set(git("diff", "--name-only", a.base).split())
    release = version(a.base) != version("HEAD")
    bad = False
    if "CHANGELOG.md" in changed and not release and touches_unreleased(a.base):
        print("This branch edits the Unreleased section of CHANGELOG.md, which only the release PR writes. Put the entry in a new file in\n"
              "changelog.d/ (see changelog.d/README.md) and undo the CHANGELOG.md edit:\n"
              f"  git checkout {a.base} -- CHANGELOG.md")
        bad = True
    if not release and not a.title.startswith("[scores]"):
        for path in SCORE_FILES:
            if path not in changed:
                continue
            added, removed = changed_scores(a.base, path)
            if added or removed:
                print(f"{path}: this branch changes published score numbers ({', '.join(removed) or '-'} -> "
                      f"{', '.join(added) or '-'}). The docs thread owns them and updates them from main's reports;\n"
                      f"leave those lines as they are (only a PR titled \"[scores] ...\" may change them).")
                bad = True
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
