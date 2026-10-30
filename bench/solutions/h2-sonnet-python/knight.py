import sys
from collections import deque
L=sys.stdin.read().split('\n')
n=int(L[0])
def pr(s):
    return (int(s[1:])-1, ord(s[0])-97)
for l in L[1:]:
    t=l.split()
    if len(t)<2: continue
    try:
        a=pr(t[0]);b=pr(t[1])
    except Exception:
        print(-1);continue
    ok=lambda p:0<=p[0]<n and 0<=p[1]<n
    if not(ok(a) and ok(b)): print(-1);continue
    D={a:0};q=deque([a])
    while q:
        x=q.popleft()
        if x==b:break
        for dx,dy in((1,2),(2,1),(-1,2),(-2,1),(1,-2),(2,-1),(-1,-2),(-2,-1)):
            y=(x[0]+dx,x[1]+dy)
            if ok(y) and y not in D:D[y]=D[x]+1;q.append(y)
    print(D.get(b,-1))
