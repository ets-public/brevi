import sys
from collections import deque
L=sys.stdin.read().split('\n')
n=int(L[0])
def sq(s): return (ord(s[0])-97,int(s[1:])-1)
for l in L[1:]:
    p=l.split()
    if len(p)<2: continue
    a,b=sq(p[0]),sq(p[1])
    ok=lambda q:0<=q[0]<n and 0<=q[1]<n
    if not ok(a) or not ok(b): print(-1);continue
    D={a:0};q=deque([a])
    while q and b not in D:
        x,y=q.popleft()
        for dx,dy in((1,2),(2,1),(-1,2),(-2,1),(1,-2),(2,-1),(-1,-2),(-2,-1)):
            z=(x+dx,y+dy)
            if ok(z) and z not in D: D[z]=D[(x,y)]+1;q.append(z)
    print(D.get(b,-1))
