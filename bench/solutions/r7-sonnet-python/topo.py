import sys,heapq
adj={};indeg={}
for l in sys.stdin:
    if '->' not in l: continue
    a,b=[x.strip() for x in l.split('->')]
    for x in (a,b):
        adj.setdefault(x,set());indeg.setdefault(x,0)
    if b not in adj[a]:
        adj[a].add(b);indeg[b]+=1
h=[n for n in indeg if indeg[n]==0];heapq.heapify(h);out=[]
while h:
    n=heapq.heappop(h);out.append(n)
    for m in adj[n]:
        indeg[m]-=1
        if indeg[m]==0: heapq.heappush(h,m)
print("cycle" if len(out)<len(indeg) else "\n".join(out))
