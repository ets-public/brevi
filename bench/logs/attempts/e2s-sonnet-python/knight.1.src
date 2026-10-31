import sys
from collections import deque
L=sys.stdin.read().splitlines();n=int(L[0])
def sq(s): return (ord(s[0])-97,int(s[1:])-1)
for l in L[1:]:
    p=l.split()
    if len(p)<2: continue
    a,b=sq(p[0]),sq(p[1])
    d={a:0};q=deque([a])
    while q:
        x,y=q.popleft()
        for dx,dy in((1,2),(2,1),(-1,2),(-2,1),(1,-2),(2,-1),(-1,-2),(-2,-1)):
            v=(x+dx,y+dy)
            if 0<=v[0]<n and 0<=v[1]<n and v not in d:
                d[v]=d[(x,y)]+1;q.append(v)
    print(d.get(b,-1))
