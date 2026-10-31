import sys
from decimal import Decimal
def c(s): return int((Decimal(s.strip())*100).to_integral_value())
def f(v):
    s='-' if v<0 else '';v=abs(v);return "%s%d.%02d"%(s,v//100,v%100)
L=sys.stdin.read().splitlines()[1:]
m={};acc={}
for l in L:
    p=l.split(',')
    if len(p)<3: continue
    a=c(p[2]);k=p[0].strip()[:7]
    i,o=m.get(k,(0,0))
    if a>0: i+=a
    else: o+=-a
    m[k]=(i,o);acc[p[1].strip()]=acc.get(p[1].strip(),0)+a
for k in sorted(m):
    i,o=m[k];print(k,f(i),f(o),f(i-o))
if acc:
    t=min(acc,key=lambda x:(-acc[x],x));print("top",t,f(acc[t]))
