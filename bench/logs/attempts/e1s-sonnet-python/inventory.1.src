import sys,csv
from decimal import Decimal,ROUND_HALF_UP
r=list(csv.reader(sys.stdin))
tot={}
for row in r[1:]:
    if len(row)<3: continue
    tot[row[0]]=tot.get(row[0],Decimal(0))+int(row[1])*Decimal(row[2])
for k,v in sorted(tot.items(),key=lambda x:(-x[1],x[0])):
    print(f"{k}: {v.quantize(Decimal('0.01'),ROUND_HALF_UP)}")
