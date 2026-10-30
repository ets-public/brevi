import sys,csv
from decimal import Decimal
r=csv.reader(sys.stdin);next(r)
t={}
for row in r:
    if len(row)<3: continue
    t[row[0]]=t.get(row[0],0)+int(row[1])*Decimal(row[2].strip())
for k,v in sorted(t.items(),key=lambda x:(-x[1],x[0])):
    print("%s: %s"%(k,format(v.quantize(Decimal('0.01')),'f')))
