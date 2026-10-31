import sys
P={')':'(',']':'[','}':'{'}
for l in sys.stdin.read().split('\n')[:-1] if False else sys.stdin.read().splitlines():
    st=[];bad=None
    for i,c in enumerate(l,1):
        if c in '([{': st.append(c)
        elif c in P:
            if not st or st[-1]!=P[c]: bad=i;break
            st.pop()
    if bad is None and st: bad=len(l)+1
    print("ok" if bad is None else "error at %d"%bad)
