import sys,csv
from decimal import Decimal
tot={}
r=csv.reader(sys.stdin.read().splitlines())
next(r,None)
for row in r:
    if len(row)<3: continue
    tot[row[0]]=tot.get(row[0],0)+int(row[1])*int((Decimal(row[2].strip())*100).to_integral_value())
for k,v in sorted(tot.items(),key=lambda x:(-x[1],x[0])):
    s='-' if v<0 else ''
    v=abs(v)
    print("%s: %s%d.%02d"%(k,s,v//100,v%100))
