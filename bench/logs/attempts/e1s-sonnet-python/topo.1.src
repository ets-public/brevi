import sys,heapq
adj={};indeg={}
for l in sys.stdin:
    if '->' not in l: continue
    a,b=[x.strip() for x in l.split('->')]
    indeg.setdefault(a,0);indeg.setdefault(b,0)
    adj.setdefault(a,set())
    if b not in adj[a]:
        adj[a].add(b);indeg[b]+=1
h=[k for k,v in indeg.items() if v==0];heapq.heapify(h);out=[]
while h:
    u=heapq.heappop(h);out.append(u)
    for v in adj.get(u,()):
        indeg[v]-=1
        if indeg[v]==0: heapq.heappush(h,v)
if len(out)<len(indeg): print("cycle")
else:
    for x in out: print(x)
