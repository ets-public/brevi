import sys
iv=sorted((int(l.split()[0]),int(l.split()[1])) for l in sys.stdin if len(l.split())>=2)
m=[]
for s,e in iv:
    if m and s<=m[-1][1]: m[-1][1]=max(m[-1][1],e)
    else: m.append([s,e])
for s,e in m: print(s,e)
print("total",sum(e-s for s,e in m))
