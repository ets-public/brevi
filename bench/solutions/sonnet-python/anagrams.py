import sys
g={}
for w in set(sys.stdin.read().split()):
    g.setdefault(''.join(sorted(w)),[]).append(w)
for x in sorted((sorted(v) for v in g.values()),key=lambda v:v[0]):
    print(' '.join(x))
