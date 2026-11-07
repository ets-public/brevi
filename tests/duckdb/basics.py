import duckdb
con = duckdb.connect()
c = con.execute("create table t(a int, b varchar, c double)")
print(c is con, con.rowcount, con.description)
con.execute("insert into t values (1,'x',1.5),(2,'y',null),(3,null,2.25)")
print(con.rowcount, con.description, con.fetchall())
con.executemany("insert into t values (?, ?, ?)", [(4,'z',0.1),(5,'w',1e20)])
print(con.rowcount, con.fetchall())
print(con.execute("select * from t order by a").fetchone())
print(con.fetchone(), con.fetchmany(2), con.fetchall(), con.fetchone(), con.fetchall())
print(con.execute("select count(*) from t").fetchone()[0]); con.complete()
r = con.sql("insert into t values (6, 'q', 3)")
print(repr(r))
print(con.sql("update t set c = 0 where a > 4"))
print(con.sql("select * from t"))
print(con.sql("select * from t where a = $x", params={"x": 2}).fetchall())
print(con.execute("select $1, $2", [1, 2]).fetchall())
print(con.execute("select 1; select 2").fetchall())
rel = con.table("t")
print(rel.columns, rel.types, len(rel), rel.shape, rel.alias)
print(rel.filter("a > 2").order("a desc").limit(2).fetchall())
print(rel.project("a, b").limit(1))
print(rel.aggregate("count(*) as n, sum(a)"))
print(rel.count("*"))
print(rel.sum("a"))

print(rel.types[0] == "INTEGER", str(rel.types[0]))
rel.show()
print(con.execute("select 1.5::float, 1/3, 2.0::double, 100000000000000000000.0::double, 'inf'::double, -0.0::double").fetchall())
print(con.sql("select 1.5::float a, 1/3 b, 2.0::double c, 100000000000000000000.0::double d, 'inf'::double e, -0.0::double f"))
print(duckdb.sql("select 42 as x").fetchall())
print(duckdb.execute("select 43").fetchall())
print(duckdb.__version__, duckdb.apilevel, duckdb.paramstyle, duckdb.threadsafety)
for q in ["select * from nope", "selec 1", "select 1/'a'", "select [1][5]::int", "insert into t values ('x', 1, 1)", "select cast('abc' as int)", "create table t(a int)"]:
    try:
        con.execute(q)
    except Exception as e:
        print(type(e).__name__, [b.__name__ for b in type(e).__mro__[1:4]], str(e))
try:
    con.execute("select ?", "abc")
except Exception as e:
    print(type(e).__name__, e)
try:
    con.execute("select ?, ?", [1])
except Exception as e:
    print(type(e).__name__, e)
con.close()
try:
    con.execute("select 1")
except Exception as e:
    print(type(e).__name__, e)
