import sys
t=sys.stdin.read().split();R,C=int(t[0]),int(t[1])
m=[[int(t[2+i*C+j]) for j in range(C)] for i in range(R)]
o=[];T,B,Lf,Rt=0,R-1,0,C-1
while T<=B and Lf<=Rt:
    for j in range(Lf,Rt+1):o.append(m[T][j])
    T+=1
    for i in range(T,B+1):o.append(m[i][Rt])
    Rt-=1
    if T<=B:
        for j in range(Rt,Lf-1,-1):o.append(m[B][j])
        B-=1
    if Lf<=Rt:
        for i in range(B,T-1,-1):o.append(m[i][Lf])
        Lf+=1
print(" ".join(map(str,o)))
print(" ".join(str(sum(m[i][j] for i in range(R))) for j in range(C)))
