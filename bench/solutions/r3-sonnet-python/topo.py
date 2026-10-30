import sys,heapq
adj={};indeg={}
for l in sys.stdin:
    if '->' not in l: continue
    a,b=[x.strip() for x in l.split('->')]
    for x in(a,b):
        adj.setdefault(x,set());indeg.setdefault(x,0)
    if b not in adj[a]:
        adj[a].add(b);indeg[b]+=1
h=[x for x in indeg if indeg[x]==0];heapq.heapify(h);out=[]
while h:
    x=heapq.heappop(h);out.append(x)
    for y in adj[x]:
        indeg[y]-=1
        if indeg[y]==0: heapq.heappush(h,y)
print("\n".join(out) if len(out)==len(indeg) else "cycle")
