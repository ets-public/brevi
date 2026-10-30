import sys
from collections import Counter

counts = Counter(w.lower() for w in sys.stdin.read().split())
ranked = sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))
for word, c in ranked[:3]:
    print(f"{word} {c}")
