import sys
from collections import defaultdict
ws=set(w.lower() for w in sys.stdin.read().split())
g=defaultdict(list)
for w in ws: g[''.join(sorted(w))].append(w)
gs=[sorted(v) for v in g.values() if len(v)>1]
gs.sort(key=lambda v:(-len(v),v[0]))
for v in gs: print(' '.join(v))
