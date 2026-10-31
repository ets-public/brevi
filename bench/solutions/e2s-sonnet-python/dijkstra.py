import sys,heapq
lines=[l.split() for l in sys.stdin if l.strip()]
fr,to=lines[-1]
adj={}
for a,b,w in lines[:-1]:
    w=int(w)
    adj.setdefault(a,[]).append((b,w));adj.setdefault(b,[]).append((a,w))
d={to:0};h=[(0,to)]
while h:
    x,u=heapq.heappop(h)
    if x>d.get(u,1e18): continue
    for v,w in adj.get(u,[]):
        if x+w<d.get(v,1e18):
            d[v]=x+w;heapq.heappush(h,(x+w,v))
if fr not in d: print("none")
else:
    print(d[fr]);path=[fr];c=fr
    while c!=to:
        c=min(v for v,w in adj[c] if v in d and w+d[v]==d[c]);path.append(c)
    print("->".join(path))
