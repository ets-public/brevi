import datetime
import os.path
rows = [["2024-01-02", "a"], ["2024-13-01", "b"]]
ok = []
for r in rows:
    d = r[0]
    try:
        datetime.datetime.strptime(d, "%Y-%m-%d")
        ok.append(r)
    except ValueError:
        pass
ok[0].append("x")
print(rows, ok, os.path.join("a", "b"))
