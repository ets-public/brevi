import sys
P={')':'(',']':'[','}':'{'}
for l in sys.stdin.read().split('\n')[:-1] if False else sys.stdin:
    l=l.rstrip('\n');s=[];ok=True
    for ch in l:
        if ch in '([{': s.append(ch)
        elif ch in P:
            if not s or s.pop()!=P[ch]: ok=False;break
    print("yes" if ok and not s else "no")
