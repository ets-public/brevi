import sys
P={')':'(',']':'[','}':'{'}
data=sys.stdin.read().split('\n')
if data and data[-1]=='': data.pop()
for l in data:
    l=l.rstrip('\r')
    s=[];res=None
    for i,ch in enumerate(l,1):
        if ch in '([{': s.append(ch)
        elif ch in P:
            if not s or s[-1]!=P[ch]:
                res=i;break
            s.pop()
    if res is None and s: res=len(l)+1
    print('ok' if res is None else f'error at {res}')
