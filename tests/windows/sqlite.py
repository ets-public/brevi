import sqlite3

con = sqlite3.connect(":memory:")
con.execute("create table t (x int, s text)")
con.executemany("insert into t values (?, ?)", [(1, "a"), (2, "b")])
print(con.execute("select x, s from t order by x").fetchall())
print(con.execute("select sum(x) from t").fetchone()[0])
con.close()
