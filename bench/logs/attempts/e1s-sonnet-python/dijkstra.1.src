import sys,heapq
lines=[l.split() for l in sys.stdin if l.strip()]
fr,to=lines[-1]
g={}
for a,b,w in lines[:-1]:
    w=int(w)
    g.setdefault(a,[]).append((b,w));g.setdefault(b,[]).append((a,w))
INF=float('inf')
dist={to:0};h=[(0,to)]
while h:
    d,u=heapq.heappop(h)
    if d>dist.get(u,INF): continue
    for v,w in g.get(u,[]):
        if d+w<dist.get(v,INF):
            dist[v]=d+w;heapq.heappush(h,(d+w,v))
if fr not in dist: print("none")
else:
    print(dist[fr])
    path=[fr];c=fr
    while c!=to:
        c=min(v for v,w in g[c] if v in dist and dist[v]+w==dist[c])
        path.append(c)
    print("->".join(path))
