import sys
from collections import Counter
c = Counter(sys.stdin.read().split())
for w, n in sorted(c.items(), key=lambda p: (-p[1], p[0]))[:10]: print(w, n)
