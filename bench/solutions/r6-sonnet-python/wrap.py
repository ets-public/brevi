import sys
L=sys.stdin.read().split('\n')
w=int(L[0].strip())
paras=[];cur=[]
for l in L[1:]:
    if l.strip()=='' :
        if cur: paras.append(cur);cur=[]
    else: cur+=l.split()
if cur: paras.append(cur)
res=[]
for ws in paras:
    lines=[];line=''
    for x in ws:
        if not line: line=x
        elif len(line)+1+len(x)<=w: line+=' '+x
        else: lines.append(line);line=x
    lines.append(line)
    res.append("\n".join(lines))
print("\n\n".join(res))
