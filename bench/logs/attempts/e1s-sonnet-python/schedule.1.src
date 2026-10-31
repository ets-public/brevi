import sys
jobs={};order=[]
for l in sys.stdin:
    p=l.split()
    if len(p)<2: continue
    jobs[p[0]]=(int(p[1]),p[3:] if len(p)>2 and p[2]=='after' else [])
bad=any(d not in jobs for _,ds in jobs.values() for d in ds)
st={};state={}
def go(n):
    if n in st: return True
    if state.get(n)==1: return False
    state[n]=1;s=0
    for d in jobs[n][1]:
        if not go(d): return False
        s=max(s,st[d]+jobs[d][0])
    st[n]=s;state[n]=2;return True
if not bad:
    for n in jobs:
        if not go(n): bad=True;break
if bad: print("invalid")
else:
    for n in sorted(jobs,key=lambda n:(st[n],n)): print(n,st[n],st[n]+jobs[n][0])
    print("total",max([st[n]+jobs[n][0] for n in jobs],default=0))
