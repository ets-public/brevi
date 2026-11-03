# The three most common words on stdin: brevi run examples/top.py < examples/input.txt
import sys
from collections import Counter

counts = Counter(sys.stdin.read().lower().split())
for word, n in counts.most_common(3):
    print(f"{word}: {n}")
