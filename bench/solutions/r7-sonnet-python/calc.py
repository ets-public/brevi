import sys,re
def ev(s):
    toks=re.findall(r'\d+|[-+*/()]',s)
    pos=0
    def peek():
        return toks[pos] if pos<len(toks) else None
    def expr():
        nonlocal pos
        v=term()
        while peek() in ('+','-'):
            o=toks[pos];pos+=1
            r=term()
            v=v+r if o=='+' else v-r
        return v
    def term():
        nonlocal pos
        v=unary()
        while peek() in ('*','/'):
            o=toks[pos];pos+=1
            r=unary()
            if o=='*': v*=r
            else:
                if r==0: raise ZeroDivisionError
                q=abs(v)//abs(r)
                v=q if (v<0)==(r<0) else -q
        return v
    def unary():
        nonlocal pos
        if peek()=='-':
            pos+=1;return -unary()
        if peek()=='+':
            pos+=1;return unary()
        if peek()=='(':
            pos+=1;v=expr();pos+=1;return v
        v=int(toks[pos]);pos+=1;return v
    return expr()
for line in sys.stdin:
    if not line.strip(): continue
    try: print(ev(line))
    except ZeroDivisionError: print("error")
