#!/usr/bin/env python3
"""Check command for typed-path tasks given to tools/ds_contrib.py --task.

  python3 tools/ds_typed_check.py TARGETS.txt [--guard GUARD.txt]

TARGETS.txt lists programs (paths under the repository, one per line) that the task should move onto the typed
build. GUARD.txt lists programs that build typed now and must stay typed. Every program is built with the current
target/release/brevi, its path is read from the {"path":...} line, and each one that built is run on its task's
tests next to python3 (bench/typed_corpus.py's shadow check). Exit 0 only when every target builds typed, every
guard program still builds typed, and no program prints something python3 does not. The output lists each program
with its path, the refusal reason and the shadow result, so the same command explains a failure.
"""
import concurrent.futures as cf, os, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "bench"))
os.environ.setdefault("BREVI", str(ROOT / "target" / "release" / "brevi"))
sys.argv.append("--shadow")  # typed_corpus reads its shadow flag from argv at import
import typed_corpus  # noqa: E402

typed_corpus.SHADOW = True


def read_list(p):
    return [l.strip() for l in Path(p).read_text().splitlines() if l.strip() and not l.startswith("#")]


def main():
    args = [a for a in sys.argv[1:] if a != "--shadow"]
    if not args:
        sys.exit(__doc__)
    targets = read_list(args[0])
    guard = read_list(args[args.index("--guard") + 1]) if "--guard" in args else []
    progs = [("target", f) for f in targets] + [("guard", f) for f in guard]
    with cf.ThreadPoolExecutor(4) as ex:
        res = list(ex.map(lambda p: (p[0], typed_corpus.one(("sol", str(ROOT / p[1]), None))), progs))
    bad = 0
    for role, r in res:
        sh = r.get("shadow", "-")
        wrong = isinstance(sh, str) and sh.startswith("BAD")
        ok = r.get("path") == "typed" and not wrong
        bad += (not ok) or wrong
        rel = os.path.relpath(r["f"], ROOT)
        why = r.get("why", "")
        print(f"{'ok ' if ok and not wrong else 'NO '} {role:6} {r.get('path'):8} shadow={sh}  {rel}" + (f"\n      why: {why}" if why else ""))
    n_t = sum(1 for role, r in res if role == "target" and r.get("path") == "typed")
    print(f"\n{n_t} of {len(targets)} targets typed; {sum(1 for role, r in res if role == 'guard' and r.get('path') == 'typed')} of {len(guard)} guard programs still typed")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
