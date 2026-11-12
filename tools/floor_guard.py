#!/usr/bin/env python3
"""Fails when a branch raises a conformance floor (compat/conformance*_floor.json "matched") or edits the published
conformance reports (bench/conformance/report.*, bench/conformance/stdlib/report.*).

Main raises the floors itself after each merge (.github/workflows/conformance-main.yml runs
tools/conformance.py --check --raise-floor on the merged tree), so PRs leave them alone: when every
conformance PR raised the same two numbers, each merge put every other open PR into conflict, and each
then merged main, re-measured and re-ran CI. The same job writes the reports. Lowering a floor on purpose is
still a PR edit.

usage: floor_guard.py BASE    (BASE: the commit the branch is compared with, e.g. HEAD^1 or a merge-base)
"""
import json, subprocess, sys

FILES = ["compat/conformance_floor.json", "compat/conformance_stdlib_floor.json"]
REPORTS = ["bench/conformance/report.md", "bench/conformance/report.json",
           "bench/conformance/stdlib/report.md", "bench/conformance/stdlib/report.json"]


def matched(rev, path):
    r = subprocess.run(["git", "show", f"{rev}:{path}"], capture_output=True, text=True)
    return json.loads(r.stdout).get("matched", {}) if r.returncode == 0 else {}


def main(base):
    bad = []
    for f in FILES:
        old = matched(base, f)
        try:
            new = json.load(open(f)).get("matched", {})
        except FileNotFoundError:
            continue
        bad += [f"{f}: {m} {old[m]} -> {n}" for m, n in new.items() if m in old and n > old[m]]
    r = subprocess.run(["git", "diff", "--name-only", base, "--", *REPORTS], capture_output=True, text=True)
    reports = r.stdout.split()
    if reports:
        print("This branch edits the published conformance reports; main rewrites them after each merge.\n"
              f"Undo with:\n  git checkout {base} -- {' '.join(reports)}")
    if bad:
        print("This branch raises a conformance floor. Leave the floors to main: after the merge,\n"
              "conformance-main.yml raises them to what the merged tree measures (PRs that each raised\n"
              "them conflicted with every other open PR). Undo the raise with:\n"
              f"  git checkout {base} -- {' '.join(FILES)}\n  " + "\n  ".join(bad))
    return 1 if bad or reports else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "HEAD^1"))
