import sqlite3

c = sqlite3.connect(":memory:")
c.execute("create table t(x)")
print(c.in_transaction)
c.execute("insert into t values (1)")
print(c.in_transaction)
c.rollback()
print(c.execute("select count(*) from t").fetchone(), c.in_transaction)
c.execute("insert into t values (2)")
c.commit()
print(c.execute("select * from t").fetchall(), c.in_transaction)
with c:
    c.execute("insert into t values (3)")
print(c.execute("select * from t").fetchall(), c.in_transaction)
try:
    with c:
        c.execute("insert into t values (4)")
        raise ValueError("boom")
except ValueError as e:
    print("caught", e)
print(c.execute("select * from t").fetchall(), c.in_transaction)
c.isolation_level = None
print(repr(c.isolation_level))
c.execute("insert into t values (5)")
print(c.in_transaction)
c.execute("begin")
c.execute("insert into t values (6)")
print(c.in_transaction)
c.execute("rollback")
print(c.execute("select * from t").fetchall())
for lvl in ["DEFERRED", "immediate", "Exclusive", ""]:
    c.isolation_level = lvl
    print(repr(c.isolation_level))
for bad in ["serializable", 5]:
    try:
        c.isolation_level = bad
    except Exception as e:
        print(type(e).__name__, e)
c.isolation_level = "IMMEDIATE"
c.execute("delete from t where x > 3")
print(c.in_transaction, c.total_changes)
c.commit()
c2 = sqlite3.connect(":memory:", autocommit=False)
print(c2.autocommit, c2.in_transaction)
c2.execute("create table q(y)")
c2.execute("insert into q values (1)")
c2.rollback()
print(c2.in_transaction)
try:
    print(c2.execute("select * from q").fetchall())
except sqlite3.OperationalError as e:
    print("gone:", e)
c2.autocommit = True
print(c2.in_transaction, c2.autocommit)
c3 = sqlite3.connect(":memory:", autocommit=True)
c3.execute("create table z(v)")
c3.execute("insert into z values (1)")
print(c3.in_transaction, c3.execute("select * from z").fetchall())
try:
    sqlite3.connect(":memory:", autocommit="yes")
except ValueError as e:
    print(e)
cur = c.cursor()
cur.execute("select x from t order by x")
print(cur.fetchone(), cur.fetchall(), cur.fetchone())
cur.executemany("insert into t values (?)", ((i,) for i in range(10, 13)))
print(cur.rowcount, cur.lastrowid)
cur.execute("insert into t values (99)")
print(cur.rowcount, cur.lastrowid)
print(sum(1 for _ in c.execute("select * from t")))
gen = ({"v": i} for i in range(3))
print(c.executemany("insert into t values (:v)", gen).rowcount)
