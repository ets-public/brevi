import sys,heapq
adj={};indeg={}
for l in sys.stdin:
    p=l.split('->')
    if len(p)<2: continue
    a,b=p[0].strip(),p[1].strip()
    for x in (a,b):
        adj.setdefault(x,set());indeg.setdefault(x,0)
    if b not in adj[a]:
        adj[a].add(b);indeg[b]+=1
h=[x for x in indeg if indeg[x]==0];heapq.heapify(h);out=[]
while h:
    u=heapq.heappop(h);out.append(u)
    for v in adj[u]:
        indeg[v]-=1
        if indeg[v]==0: heapq.heappush(h,v)
print("\n".join(out) if len(out)==len(indeg) else "cycle")
