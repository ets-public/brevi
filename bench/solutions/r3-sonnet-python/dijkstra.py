import sys,heapq
L=[l.split() for l in sys.stdin if l.strip()]
s,t=L[-1];g={}
for a,b,w in L[:-1]:
    w=int(w);g.setdefault(a,{});g.setdefault(b,{})
    g[a][b]=min(w,g[a].get(b,w));g[b][a]=g[a][b]
g.setdefault(s,{});g.setdefault(t,{})
D={t:0};h=[(0,t)]
while h:
    d,u=heapq.heappop(h)
    if d>D[u]: continue
    for v,w in g[u].items():
        if d+w<D.get(v,1e18): D[v]=d+w;heapq.heappush(h,(d+w,v))
if s not in D: print("none")
else:
    print(D[s]);p=[s];u=s
    while u!=t:
        u=min(v for v,w in g[u].items() if v in D and w+D[v]==D[u]);p.append(u)
    print("->".join(p))
