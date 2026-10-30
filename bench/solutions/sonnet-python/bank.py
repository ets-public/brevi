import sys
a={}
for l in sys.stdin:
    p=l.split()
    if not p: continue
    n=p[1]
    if p[0]=="deposit": a[n]=a.get(n,0)+int(p[2])
    elif p[0]=="withdraw":
        if int(p[2])>a.get(n,0): print("error: insufficient funds for "+n)
        else: a[n]=a.get(n,0)-int(p[2])
    elif p[0]=="balance": print(n,a.get(n,0))
