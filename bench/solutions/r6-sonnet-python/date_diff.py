import sys
from datetime import date
for l in sys.stdin:
    p=l.split()
    if len(p)>=2:
        a,b=[date(*map(int,x.split('-'))) for x in p[:2]]
        print(abs((a-b).days))
