import sys
r=[]
for l in sys.stdin:
    p=l.split()
    if len(p)>=2: r.append((p[0],int(p[1])))
s=sorted(x[1] for x in r);n=len(s)
m=s[n//2] if n%2 else (s[n//2-1]+s[n//2])/2
print("%.1f"%m)
mx=max(s)
for nm in sorted(x[0] for x in r if x[1]==mx): print(nm)
