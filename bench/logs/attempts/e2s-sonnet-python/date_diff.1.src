import sys
from datetime import date
for l in sys.stdin:
    p=l.split()
    if len(p)<2: continue
    a=date(*map(int,p[0].split('-')));b=date(*map(int,p[1].split('-')))
    print(abs((a-b).days))
