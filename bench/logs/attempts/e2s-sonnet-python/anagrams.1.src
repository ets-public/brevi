import sys
g={}
for w in set(x.lower() for x in sys.stdin.read().split()):
    g.setdefault(''.join(sorted(w)),[]).append(w)
r=[sorted(v) for v in g.values() if len(v)>1]
r.sort(key=lambda v:(-len(v),v[0]))
for v in r: print(" ".join(v))
