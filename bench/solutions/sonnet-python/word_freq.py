import sys
from collections import Counter
c=Counter(sys.stdin.read().lower().split())
for w,n in sorted(c.items(),key=lambda x:(-x[1],x[0]))[:3]:
    print(w,n)
