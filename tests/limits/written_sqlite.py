# SQLite writes its database itself, where Brevi cannot count the bytes: under --limit written it writes no file
# (the database opens read-only), and an in-memory database works as usual.
import sqlite3

m = sqlite3.connect(":memory:")
m.execute("create table t (x)")
m.executemany("insert into t values (?)", [(i,) for i in range(1000)])
print(m.execute("select count(*), sum(x) from t order by x desc").fetchone())
try:
    c = sqlite3.connect("limits-written.out")
    c.execute("create table t (x)")
    print("written")
except sqlite3.OperationalError as e:
    print("refused:", e)
