import sys
class Z(Exception): pass
def ev(s):
    t=[];i=0
    while i<len(s):
        c=s[i]
        if c.isspace(): i+=1
        elif c.isdigit():
            j=i
            while j<len(s) and s[j].isdigit(): j+=1
            t.append(int(s[i:j]));i=j
        else: t.append(c);i+=1
    pos=[0]
    def pk(): return t[pos[0]] if pos[0]<len(t) else None
    def expr():
        v=term()
        while pk() in ('+','-'):
            o=t[pos[0]];pos[0]+=1;r=term()
            v=v+r if o=='+' else v-r
        return v
    def term():
        v=un()
        while pk() in ('*','/'):
            o=t[pos[0]];pos[0]+=1;r=un()
            if o=='*': v*=r
            else:
                if r==0: raise Z()
                q=abs(v)//abs(r)
                v=q if (v<0)==(r<0) else -q
        return v
    def un():
        x=pk()
        if x=='-': pos[0]+=1;return -un()
        if x=='+': pos[0]+=1;return un()
        if x=='(':
            pos[0]+=1;v=expr();pos[0]+=1;return v
        pos[0]+=1;return x
    return expr()
for l in sys.stdin:
    if not l.strip(): continue
    try: print(ev(l))
    except Z: print("error")
