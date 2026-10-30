import sys,heapq
L=[l.split() for l in sys.stdin if l.strip()]
fr,to=L[-1];g={}
for a,b,w in L[:-1]:
    w=int(w);g.setdefault(a,[]).append((b,w));g.setdefault(b,[]).append((a,w))
d={to:0};h=[(0,to)]
while h:
    x,u=heapq.heappop(h)
    if x>d.get(u,1e30):continue
    for v,w in g.get(u,[]):
        if x+w<d.get(v,1e30): d[v]=x+w;heapq.heappush(h,(x+w,v))
if fr not in d: print("none")
else:
    p=[fr];c=fr
    while c!=to:
        c=min(v for v,w in g[c] if v in d and w+d[v]==d[c]);p.append(c)
    print(d[fr]);print("->".join(p))
