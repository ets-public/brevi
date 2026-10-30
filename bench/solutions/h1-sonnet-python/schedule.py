import sys
jobs={};order=[]
for l in sys.stdin:
    p=l.split()
    if not p: continue
    jobs[p[0]]=(int(p[1]),p[3:] if len(p)>2 and p[2]=='after' else [])
bad=any(d not in jobs for _,ds in jobs.values() for d in ds)
end={};state={}
def go(n):
    if n in end: return end[n]
    if state.get(n)==1: raise ValueError
    state[n]=1
    dur,ds=jobs[n]
    s=max([go(d) for d in ds],default=0)
    state[n]=2
    end[n]=s+dur
    return end[n]
if not bad:
    sys.setrecursionlimit(10000)
    try:
        for n in jobs: go(n)
    except ValueError: bad=True
if bad: print('invalid')
else:
    r=sorted((end[n]-jobs[n][0],n) for n in jobs)
    for s,n in r: print(n,s,end[n])
    print('total',max(end.values(),default=0))
