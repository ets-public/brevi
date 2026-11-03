import csv
import os
import sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
rows = ["name,dept,salary", "ann,eng,100", "", "bob,ops,70", "cy,eng,51"]
by = {}
for r in csv.DictReader(rows):
    by.setdefault(r["dept"], []).append(float(r["salary"]))
for k in sorted(by):
    print(k, "%.2f" % (sum(by[k]) / len(by[k])))
first = list(csv.DictReader(rows))[0]
print(first, list(first.keys()))
print(__file__.endswith("csv_file.py"), __file__.startswith("/"))
for i in [5, -5, 1234567]:
    print(f"{i:06d}|{i:6}|{i:d}|{i:03}|{i:>8d}|{i:<6d}|{i:+05d}")
print("a,b,,c".split(","), "x:y".split(":"), "a--b".split("--"))
