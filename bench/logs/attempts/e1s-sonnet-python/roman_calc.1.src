import sys
V={'I':1,'V':5,'X':10,'L':50,'C':100,'D':500,'M':1000}
def r2i(s):
    t=0
    for i,c in enumerate(s):
        v=V[c]
        if i+1<len(s) and V[s[i+1]]>v: t-=v
        else: t+=v
    return t
def i2r(n):
    o=""
    for v,s in((1000,'M'),(900,'CM'),(500,'D'),(400,'CD'),(100,'C'),(90,'XC'),(50,'L'),(40,'XL'),(10,'X'),(9,'IX'),(5,'V'),(4,'IV'),(1,'I')):
        while n>=v: o+=s;n-=v
    return o
for l in sys.stdin:
    p=l.split()
    if not p: continue
    t=r2i(p[0])
    for i in range(1,len(p)-1,2):
        x=r2i(p[i+1]);t+=x if p[i]=='+' else -x
    print(i2r(t) if t>0 else "nulla")
