import sys
V={'I':1,'V':5,'X':10,'L':50,'C':100,'D':500,'M':1000}
def p(s):
    t=0
    for i,c in enumerate(s):
        v=V[c]
        if i+1<len(s) and V[s[i+1]]>v: t-=v
        else: t+=v
    return t
def r(n):
    o=''
    for v,s in [(1000,'M'),(900,'CM'),(500,'D'),(400,'CD'),(100,'C'),(90,'XC'),(50,'L'),(40,'XL'),(10,'X'),(9,'IX'),(5,'V'),(4,'IV'),(1,'I')]:
        while n>=v:o+=s;n-=v
    return o
for l in sys.stdin:
    t=l.split()
    if not t: continue
    tot=p(t[0])
    for i in range(1,len(t)-1,2):
        x=p(t[i+1])
        tot+= x if t[i]=='+' else -x
    print(r(tot) if tot>0 else 'nulla')
