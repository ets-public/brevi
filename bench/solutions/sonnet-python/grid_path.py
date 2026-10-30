import sys
from collections import deque
g=[l.rstrip('\n') for l in sys.stdin if l.strip()]
s=e=None
for i,r in enumerate(g):
    for j,c in enumerate(r):
        if c=='S': s=(i,j)
        if c=='E': e=(i,j)
if not s or not e: print(-1)
else:
    d={s:0};q=deque([s]);res=-1
    while q:
        x,y=q.popleft()
        if (x,y)==e: res=d[e];break
        for a,b in((1,0),(-1,0),(0,1),(0,-1)):
            u,v=x+a,y+b
            if 0<=u<len(g) and 0<=v<len(g[u]) and g[u][v]!='#' and (u,v) not in d:
                d[(u,v)]=d[(x,y)]+1;q.append((u,v))
    print(res)
