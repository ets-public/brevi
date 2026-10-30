import sys,re
d=sys.stdin.read().split('\n',1)
w=int(d[0].strip());t=d[1] if len(d)>1 else ''
out=[]
for para in re.split(r'\n\s*\n',t):
    ws=para.split()
    if not ws: continue
    ls=[];cur=''
    for x in ws:
        if not cur: cur=x
        elif len(cur)+1+len(x)<=w: cur+=' '+x
        else: ls.append(cur);cur=x
    ls.append(cur)
    out.append("\n".join(ls))
print("\n\n".join(out))
