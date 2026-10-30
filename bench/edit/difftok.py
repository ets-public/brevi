# Estimated tokens of the lines in NEW that are not in OLD (added or changed lines).
import difflib, re, sys
old, new = open(sys.argv[1]).read().splitlines(), open(sys.argv[2]).read().splitlines()
added = [l for l in difflib.unified_diff(old, new, lineterm="", n=0) if l.startswith("+") and not l.startswith("+++")]
print(len(re.findall(r"[A-Za-z]+|\d+|\s{2,}|[^\sA-Za-z\d]", "\n".join(l[1:] for l in added))))
