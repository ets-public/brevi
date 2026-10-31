import sys
J={};order=[]
for l in sys.stdin:
    p=l.split()
    if not p: continue
    J[p[0]]=(int(p[1]),p[3:] if len(p)>2 and p[2]=='after' else [])
sys.setrecursionlimit(100000)
end={};st={};bad=False
def go(n,vis):
    global bad
    if n in end: return end[n]
    if n in vis or n not in J: bad=True;return 0
    vis.add(n);s=0
    for d in J[n][1]: s=max(s,go(d,vis))
    vis.discard(n);st[n]=s;end[n]=s+J[n][0];return end[n]
for n in J:
    go(n,set())
    if bad: break
if bad: print("invalid")
else:
    for n in sorted(J,key=lambda x:(st[x],x)): print(n,st[n],end[n])
    print("total",max(end.values(),default=0))
