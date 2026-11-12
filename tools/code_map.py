#!/usr/bin/env python3
"""Print a map of the code: one line per file, its path and the first line of its top comment.

An agent starting a task greps this instead of reading through 130k lines of src/ to find its way.
It is generated on demand, never committed, so it can't go stale or put PRs into conflict.

  code_map.py [PREFIX...]     only paths starting with a prefix, e.g. src/dynpy tools
  code_map.py --grep WORD     only files whose path or summary mentions WORD (case-insensitive)
  code_map.py --check         exit 1 if a src/ file has no top comment (CI runs this)

A Rust file's summary is its first comment line (`//` or `//!`, banner dashes stripped); a Python
file's is the first line of its docstring or first comment. Each line ends with the file's size in
thousands of lines, so a reader knows when to grep a file rather than read it.
"""
import argparse, ast, os, re, sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
DIRS = [("src", ".rs"), ("tools", ".py"), ("bench", ".py"), ("tests", ".py")]


def rust_summary(text):
    for line in text.splitlines():
        s = line.strip()
        if not s:
            continue
        if not s.startswith("//"):
            return ""
        s = re.sub(r"^//[!/]?\s*", "", s).strip(" -")
        if s:
            return s
    return ""


def py_summary(text):
    try:
        doc = ast.get_docstring(ast.parse(text))
    except (SyntaxError, ValueError):
        doc = None
    if doc:
        return doc.strip().splitlines()[0]
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("#!") or not s:
            continue
        return s.lstrip("# ").strip() if s.startswith("#") else ""
    return ""


def files():
    for d, ext in DIRS:
        top = os.path.join(ROOT, d)
        depth = None if d == "src" else 0  # src in full; elsewhere only the top folder
        for dirpath, dirnames, names in os.walk(top):
            rel = os.path.relpath(dirpath, ROOT)
            dirnames[:] = sorted(n for n in dirnames if n not in ("work", "out", "__pycache__", "logs"))
            if depth == 0 and dirpath != top:
                dirnames[:] = []
                continue
            for n in sorted(names):
                if n.endswith(ext):
                    yield os.path.join(rel, n)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("prefix", nargs="*")
    ap.add_argument("--grep")
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    missing = []
    for rel in files():
        if a.prefix and not any(rel.startswith(p.rstrip("/")) for p in a.prefix):
            continue
        text = open(os.path.join(ROOT, rel), encoding="utf-8", errors="ignore").read()
        summary = rust_summary(text) if rel.endswith(".rs") else py_summary(text)
        if a.check:
            if rel.startswith("src/") and not summary:
                missing.append(rel)
            continue
        line = f"{rel}: {summary or '(no summary)'} [{text.count(chr(10)) / 1000:.1f}k]"
        if a.grep and a.grep.lower() not in line.lower():
            continue
        print(line)
    if missing:
        print("These src/ files have no top comment saying what they hold (tools/code_map.py prints it for agents):\n  "
              + "\n  ".join(missing))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
