# Point queries, row iteration and inserts on an in-memory sqlite3 database.
import sqlite3
import sys
import time

n = int(sys.argv[1]) if len(sys.argv) > 1 else 50000
con = sqlite3.connect(":memory:")
con.execute("create table t (id integer primary key, name text, v real)")
t = time.perf_counter()
for i in range(n):
    con.execute("insert into t values (?, ?, ?)", (i, "n%d" % i, i * 0.25))
t1 = time.perf_counter()
s = 0.0
for i in range(n):
    row = con.execute("select name, v from t where id = ?", (i * 7 % n,)).fetchone()
    s += row[1] + len(row[0])
t2 = time.perf_counter()
c = 0
for row in con.execute("select id, name, v from t"):
    c += row[0]
t3 = time.perf_counter()
print(s, c)
print("time %.3f insert %.3f point %.3f iter %.3f" % (t3 - t, t1 - t, t2 - t1, t3 - t2), file=sys.stderr)
