import sys,csv
from decimal import Decimal
r=list(csv.reader(sys.stdin))
m={};acc={}
for row in r[1:]:
    if len(row)<3: continue
    a=Decimal(row[2].strip());k=row[0].strip()[:7]
    i,o=m.setdefault(k,[Decimal(0),Decimal(0)])
    if a>0: m[k][0]+=a
    else: m[k][1]+=-a
    acc[row[1].strip()]=acc.get(row[1].strip(),Decimal(0))+a
for k in sorted(m):
    i,o=m[k];print(f"{k} {i:.2f} {o:.2f} {i-o:.2f}")
if acc:
    n=min(acc,key=lambda a:(-acc[a],a));print(f"top {n} {acc[n]:.2f}")
