import sqlite3

con = sqlite3.connect(":memory:")
for sql in ["attach '/tmp/brevi_sb_attach.db' as o", "attach 'file:/tmp/brevi_sb_attach.db?vfs=unix' as o", "vacuum into '/tmp/brevi_sb_vacuum.db'", "select load_extension('/tmp/x.so')"]:
    try:
        con.execute(sql)
        print("ran", sql)
    except sqlite3.OperationalError as e:
        print("refused", e)
con.execute("create table t(x)")
con.executemany("insert into t values (?)", [(i,) for i in range(1000)])
print(con.execute("select count(*), sum(x) from t").fetchone())
print(con.execute("select x from t order by x desc limit 2").fetchall())
