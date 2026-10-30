import sys
jobs={};order=[]
for l in sys.stdin:
    p=l.split()
    if not p: continue
    deps=p[3:] if len(p)>2 and p[2]=='after' else []
    jobs[p[0]]=(int(p[1]),deps)
bad=any(d not in jobs for _,ds in jobs.values() for d in ds)
end={};
if not bad:
    st={}
    rem=dict(jobs)
    while rem:
        prog=False
        for n in list(rem):
            du,ds=rem[n]
            if all(d in end for d in ds):
                s=max([end[d] for d in ds],default=0)
                st[n]=s;end[n]=s+du;del rem[n];prog=True
        if not prog: bad=True;break
if bad: print('invalid')
else:
    for n in sorted(st,key=lambda n:(st[n],n)): print(n,st[n],end[n])
    print('total',max(end.values(),default=0))
