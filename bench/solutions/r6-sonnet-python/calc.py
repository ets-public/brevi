import sys
def ev(s):
    t=[];i=0
    while i<len(s):
        c=s[i]
        if c.isspace(): i+=1
        elif c.isdigit():
            j=i
            while j<len(s) and s[j].isdigit(): j+=1
            t.append(int(s[i:j])); i=j
        else: t.append(c); i+=1
    p=[0]
    def expr():
        v=term()
        while p[0]<len(t) and t[p[0]] in '+-':
            o=t[p[0]];p[0]+=1;r=term()
            v=v+r if o=='+' else v-r
        return v
    def term():
        v=unary()
        while p[0]<len(t) and t[p[0]] in '*/':
            o=t[p[0]];p[0]+=1;r=unary()
            if o=='*': v*=r
            else:
                if r==0: raise ZeroDivisionError
                q=abs(v)//abs(r)
                v=q if (v<0)==(r<0) else -q
        return v
    def unary():
        x=t[p[0]]
        if x=='-': p[0]+=1; return -unary()
        if x=='+': p[0]+=1; return unary()
        if x=='(':
            p[0]+=1;v=expr();p[0]+=1;return v
        p[0]+=1;return x
    return expr()
for l in sys.stdin:
    if not l.strip(): continue
    try: print(ev(l))
    except ZeroDivisionError: print("error")
