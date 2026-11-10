import duckdb

con = duckdb.connect()
con.execute("create table t (x int, s varchar)")
con.execute("insert into t values (1, 'a'), (2, 'b')")
print(con.execute("select x, s from t order by x").fetchall())
print(con.execute("select sum(x) from t").fetchone()[0])
con.executemany("insert into t values (?, ?)", [(3, "c"), (4, None)])
print(con.execute("select count(*), max(x) from t").fetchone())
con.close()
