import sys
from collections import defaultdict
g=defaultdict(set)
for w in sys.stdin.read().split():
    w=w.lower(); g[''.join(sorted(w))].add(w)
r=[sorted(s) for s in g.values() if len(s)>1]
r.sort(key=lambda x:(-len(x),x[0]))
for x in r: print(' '.join(x))
