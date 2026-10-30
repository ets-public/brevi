import sys,heapq
L=[l.split() for l in sys.stdin if l.strip()]
src,dst=L[-1]
g={}
for a,b,w in L[:-1]:
    w=int(w)
    g.setdefault(a,[]).append((b,w));g.setdefault(b,[]).append((a,w))
g.setdefault(src,[]);g.setdefault(dst,[])
dist={dst:0};h=[(0,dst)]
while h:
    d,u=heapq.heappop(h)
    if d>dist[u]: continue
    for v,w in g[u]:
        if v not in dist or d+w<dist[v]:
            dist[v]=d+w;heapq.heappush(h,(d+w,v))
if src not in dist: print("none")
else:
    print(dist[src]);p=[src];c=src
    while c!=dst:
        c=min(v for v,w in g[c] if v in dist and w+dist[v]==dist[c]);p.append(c)
    print("->".join(p))
