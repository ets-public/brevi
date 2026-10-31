import sys
g={}
for w in set(x.lower() for x in sys.stdin.read().split()):
    g.setdefault(''.join(sorted(w)),[]).append(w)
gs=[sorted(v) for v in g.values() if len(v)>1]
gs.sort(key=lambda v:(-len(v),v[0]))
for v in gs: print(" ".join(v))
