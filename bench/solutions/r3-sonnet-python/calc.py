import sys
def ev(s):
    t=[];i=0
    while i<len(s):
        c=s[i]
        if c.isdigit():
            j=i
            while j<len(s) and s[j].isdigit(): j+=1
            t.append(int(s[i:j]));i=j;continue
        if c in '+-*/()': t.append(c)
        i+=1
    p=0
    def expr():
        nonlocal p
        v=term()
        while p<len(t) and t[p] in('+','-'):
            o=t[p];p+=1;r=term()
            v=v+r if o=='+' else v-r
        return v
    def term():
        nonlocal p
        v=un()
        while p<len(t) and t[p] in('*','/'):
            o=t[p];p+=1;r=un()
            if o=='*': v*=r
            else:
                if r==0: raise ZeroDivisionError
                q=abs(v)//abs(r)
                v=q if (v<0)==(r<0) else -q
        return v
    def un():
        nonlocal p
        if t[p]=='-': p+=1; return -un()
        if t[p]=='+': p+=1; return un()
        if t[p]=='(':
            p+=1;v=expr();p+=1;return v
        v=t[p];p+=1;return v
    return expr()
for l in sys.stdin:
    if not l.strip(): continue
    try: print(ev(l))
    except ZeroDivisionError: print("error")
