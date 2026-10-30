import sys,csv
d={}
for r in csv.DictReader(sys.stdin):
    d.setdefault(r['dept'],[]).append(float(r['salary']))
for k in sorted(d):
    print(k,"%.2f"%(sum(d[k])/len(d[k])))
