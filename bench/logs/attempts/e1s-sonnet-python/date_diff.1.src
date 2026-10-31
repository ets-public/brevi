import sys
from datetime import date
def p(s):
    y,m,d=map(int,s.split('-'));return date(y,m,d).toordinal()
for l in sys.stdin:
    a=l.split()
    if len(a)==2: print(abs(p(a[0])-p(a[1])))
