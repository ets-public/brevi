"""Find plain Python that Brevi runs differently from CPython.

Each probe in parity/probes.py is a short program. It is run with python3 and with `brevi run`, and the
outputs are compared (stdout plus the last line of stderr, so a CPython traceback compares by its
exception line). A probe is SAME, DIFF (Brevi printed something else: a silent wrong answer, the
worst kind, because a model has to debug it), LOUD (Brevi stopped with an error where CPython ran),
or OK-ERR (both failed). Every probe starts with `import sys` so Brevi reads it as Python.

    python3 tools/parity_probe.py            # all probes
    python3 tools/parity_probe.py -k dict    # probes whose name contains "dict"
    python3 tools/parity_probe.py --json     # one JSON line per probe
"""
import argparse, json, os, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent / "parity"))
from probes import PROBES  # noqa: E402

BREVI = os.environ.get("BREVI", str(ROOT / "target" / "release" / "brevi"))


def run(cmd, src, stdin):
    d = tempfile.mkdtemp()
    p = Path(d) / "main.py"
    p.write_text("import sys\n" + src.strip() + "\n")
    try:
        r = subprocess.run(cmd + [str(p)], input=stdin, capture_output=True, text=True, timeout=120, cwd=d)
    except subprocess.TimeoutExpired:
        return "timeout", 1
    err = r.stderr.strip().splitlines()
    return (r.stdout.rstrip("\n") + ("\n" + err[-1] if err else "")).strip(), r.returncode


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("-k", default="")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    counts = {}
    for name, src, *rest in PROBES:
        if a.k not in name:
            continue
        stdin = rest[0] if rest else ""
        py, prc = run([sys.executable], src, stdin)
        bv, brc = run([BREVI, "run"], src, stdin)
        if py == bv:
            kind = "SAME"
        elif prc and brc:
            kind = "OK-ERR"
        elif brc and not prc:
            kind = "LOUD"
        else:
            kind = "DIFF"
        counts[kind] = counts.get(kind, 0) + 1
        if a.json:
            print(json.dumps({"probe": name, "kind": kind, "python": py, "brevi": bv}))
        elif kind != "SAME":
            print(f"{kind:6} {name}\n  python: {py!r}\n  brevi:  {bv!r}")
    if not a.json:
        print(" ".join(f"{k} {v}" for k, v in sorted(counts.items())))


if __name__ == "__main__":
    main()
