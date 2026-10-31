import sys
L=sys.stdin.read().split('\n')
W=int(L[0]);paras=[];cur=[]
for l in L[1:]:
    if l.strip(): cur+=l.split()
    elif cur: paras.append(cur);cur=[]
if cur: paras.append(cur)
out=[]
for p in paras:
    lines=[];c=""
    for w in p:
        if not c: c=w
        elif len(c)+1+len(w)<=W: c+=" "+w
        else: lines.append(c);c=w
    lines.append(c);out.append("\n".join(lines))
print("\n\n".join(out))
