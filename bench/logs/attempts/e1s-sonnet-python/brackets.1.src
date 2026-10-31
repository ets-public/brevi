import sys
P={')':'(',']':'[','}':'{'}
for l in sys.stdin.read().split('\n')[:-1] if False else sys.stdin.read().splitlines():
    st=[];bad=None
    for i,c in enumerate(l,1):
        if c in '([{': st.append(c)
        elif c in P:
            if st and st[-1]==P[c]: st.pop()
            else: bad=i;break
    if bad: print("error at",bad)
    elif st: print("error at",len(l)+1)
    else: print("ok")
