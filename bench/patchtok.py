"""Exact tokens a retry costs when only the changed lines are rewritten (as with an edit tool):
the removed lines (quoted to find them) plus the added ones, never more than the whole new file.
usage: patchtok.py OLD NEW"""
import difflib, sys
from tokcount import count
old, new = open(sys.argv[1]).read().splitlines(), open(sys.argv[2]).read().splitlines()
diff = [l for l in difflib.unified_diff(old, new, lineterm="", n=0) if l[:1] in "+-" and l[:3] not in ("+++", "---")]
gone = "\n".join(l[1:] for l in diff if l[0] == "-")
added = "\n".join(l[1:] for l in diff if l[0] == "+")
print(min(count(gone) + count(added), count("\n".join(new))))
