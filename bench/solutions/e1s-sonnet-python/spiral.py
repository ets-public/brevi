import sys
t=sys.stdin.read().split();R,C=int(t[0]),int(t[1])
m=[[int(t[2+i*C+j]) for j in range(C)] for i in range(R)]
o=[];T,B,L,Rr=0,R-1,0,C-1
while T<=B and L<=Rr:
    for j in range(L,Rr+1): o.append(m[T][j])
    T+=1
    for i in range(T,B+1): o.append(m[i][Rr])
    Rr-=1
    if T<=B:
        for j in range(Rr,L-1,-1): o.append(m[B][j])
        B-=1
    if L<=Rr:
        for i in range(B,T-1,-1): o.append(m[i][L])
        L+=1
print(*o)
print(*[sum(m[i][j] for i in range(R)) for j in range(C)])
