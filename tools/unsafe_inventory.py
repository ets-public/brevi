#!/usr/bin/env python3
"""Where Brevi uses `unsafe`: one line per file with its count, for docs/SECURITY_TESTING.md.

cargo geiger counts only code compiled into the brevi binary, but most of Brevi's unsafe code is runtime source that
is pasted into the programs it builds (include_str!: the CPython bridge, DuckDB, ONNX Runtime, SQLite, the network
layer), so this counts the source text itself and says which side each file is on.

    python3 tools/unsafe_inventory.py            # table, largest first
    python3 tools/unsafe_inventory.py --total    # one number
"""
import re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORD = re.compile(r"\bunsafe\b")


def pasted():
    """Files the compiler pastes into generated programs (named in an include_str! or include!)."""
    out = set()
    for f in (ROOT / "src").rglob("*.rs"):
        for m in re.finditer(r'include(?:_str)?!\(\s*"([^"]+\.rs)"', f.read_text(encoding="utf-8")):
            out.add((f.parent / m.group(1)).resolve())
    return out


def main():
    paste = pasted()
    rows = []
    for f in sorted((ROOT / "src").rglob("*.rs")) + sorted((ROOT / "runtime").rglob("*.rs")):
        n = 0
        for line in f.read_text(encoding="utf-8").splitlines():
            code = line.split("//", 1)[0]
            n += len(WORD.findall(code))
        if n:
            side = "runtime" if f.resolve() in paste or "runtime" in f.parts else "compiler"
            rows.append((n, str(f.relative_to(ROOT)), side))
    rows.sort(key=lambda r: (-r[0], r[1]))
    if "--total" in sys.argv:
        print(sum(r[0] for r in rows))
        return 0
    for n, f, side in rows:
        print(f"{n:5}  {side:8}  {f}")
    print(f"{sum(r[0] for r in rows):5}  total, {sum(r[0] for r in rows if r[2] == 'runtime')} in code pasted into programs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
