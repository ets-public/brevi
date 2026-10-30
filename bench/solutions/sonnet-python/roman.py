import sys
T=[(1000,'M'),(900,'CM'),(500,'D'),(400,'CD'),(100,'C'),(90,'XC'),(50,'L'),(40,'XL'),(10,'X'),(9,'IX'),(5,'V'),(4,'IV'),(1,'I')]
for l in sys.stdin:
    if not l.strip(): continue
    n=int(l);r=''
    for v,s in T:
        while n>=v: r+=s;n-=v
    print(r)
