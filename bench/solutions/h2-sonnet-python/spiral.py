import sys
t=sys.stdin.read().split()
R,C=int(t[0]),int(t[1])
m=[[int(t[2+i*C+j]) for j in range(C)] for i in range(R)]
o=[]
a,b,c,d=0,R-1,0,C-1
while a<=b and c<=d:
    for j in range(c,d+1):o.append(m[a][j])
    for i in range(a+1,b+1):o.append(m[i][d])
    if a<b:
        for j in range(d-1,c-1,-1):o.append(m[b][j])
    if c<d:
        for i in range(b-1,a,-1):o.append(m[i][c])
    a+=1;b-=1;c+=1;d-=1
print(' '.join(map(str,o)))
print(' '.join(str(sum(m[i][j] for i in range(R))) for j in range(C)))
