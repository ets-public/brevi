import sys
P={')':'(',']':'[','}':'{'}
for line in sys.stdin:
    line=line.rstrip('\r\n')
    st=[];res=None
    for i,ch in enumerate(line,1):
        if ch in '([{': st.append(ch)
        elif ch in P:
            if not st or st[-1]!=P[ch]:
                res=i;break
            st.pop()
    if res is None and st: res=len(line)+1
    print('ok' if res is None else f'error at {res}')
