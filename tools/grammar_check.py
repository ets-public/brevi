"""Parses every known-good Brevi program with grammar/brevi.lark. usage: grammar_check.py [--gbnf] [FILE...]

With --gbnf it parses with grammar/brevi.gbnf instead, turned back into a Lark grammar, to test the llama.cpp copy.

Programs: tests/run/*.bv and every Brevi solution in compat/baseline.txt (the ones CI re-checks).
Files with `rust fn` or `py fn` are skipped, since their bodies are foreign code. Exits 1 if any fails.
Needs `pip install lark`.
"""
import multiprocessing, re, sys, time
from pathlib import Path
from lark import Lark

ROOT = Path(__file__).resolve().parent.parent


def gbnf_to_lark(text):
    """GBNF rules as Lark rules: literals and character classes become terminals, `-` in names becomes `_`."""
    tok = re.compile(r'\s+|"(?:\\.|[^"\\])*"|\[(?:\\.|[^\]\\])*\]|[a-z0-9-]+|::=|[()|?*+]')
    out = []
    for line in text.splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        name, body = line.split("::=", 1)
        parts = []
        for t in tok.findall(body):
            if t.isspace():
                continue
            if t.startswith("["):
                t = "/" + t.replace("/", "\\/") + "/"
            elif re.match(r"[a-z0-9-]+$", t):
                t = "r_" + t.replace("-", "_")
            parts.append(t)
        out.append(f"r_{name.strip().replace('-', '_')}: " + " ".join(parts))
    return "\n".join(out)


args = sys.argv[1:]
if "--gbnf" in args:
    args.remove("--gbnf")
    parser = Lark(gbnf_to_lark((ROOT / "grammar" / "brevi.gbnf").read_text()), start="r_root", parser="earley", lexer="dynamic")
else:
    parser = Lark((ROOT / "grammar" / "brevi.lark").read_text(), parser="earley", lexer="dynamic_complete")

if args:
    files = [Path(a) for a in args]
else:
    files = sorted((ROOT / "tests" / "run").glob("*.bv"))
    files += [ROOT / l for l in (ROOT / "compat" / "baseline.txt").read_text().split() if l.endswith((".bv", ".ax"))]
foreign = re.compile(r"^\s*(rust|py)\s+fn\b", re.M)


def check(f):
    t = time.time()
    try:
        parser.parse(f.read_text())
        return f, None, time.time() - t
    except Exception as e:
        return f, str(e).splitlines()[0][:160], time.time() - t


if __name__ == "__main__":
    files = [f for f in files if f.is_file() and not foreign.search(f.read_text())]
    bad, slow, t0 = [], [], time.time()
    with multiprocessing.Pool() as pool:
        for f, err, dt in pool.imap_unordered(check, files):
            name = f.relative_to(ROOT) if f.is_absolute() else f
            if err:
                bad.append(f)
                print(f"{name}: {err}", flush=True)
            if dt > 10:
                slow.append(f"{name} ({dt:.0f}s)")
    if slow:
        print("slow:", ", ".join(slow))
    print(f"{len(files) - len(bad)} of {len(files)} programs parse ({time.time() - t0:.0f}s)")
    sys.exit(1 if bad else 0)
