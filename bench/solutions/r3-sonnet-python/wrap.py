import sys
L=sys.stdin.read().split('\n')
W=int(L[0]);paras=[];cur=[]
for l in L[1:]:
    if l.strip()=='' :
        if cur: paras.append(cur);cur=[]
    else: cur+=l.split()
if cur: paras.append(cur)
out=[]
for ws in paras:
    lines=[];line=''
    for w in ws:
        if not line: line=w
        elif len(line)+1+len(w)<=W: line+=' '+w
        else: lines.append(line);line=w
    lines.append(line)
    out.append("\n".join(lines))
print("\n\n".join(out))
