import re, sys
# Rough BPE-like token estimate: words, numbers, runs of 2+ spaces, single symbols.
print(len(re.findall(r"[A-Za-z]+|\d+|\s{2,}|[^\sA-Za-z\d]", sys.stdin.read())))
