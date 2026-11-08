# sqlite3 under --allow-read / --allow-write DIR: database files (and ATTACH) only inside the granted folders.
import sqlite3

c = sqlite3.connect("tests/sandbox/scoped/out/a.db")
c.execute("drop table if exists t")
c.execute("create table t(x)")
c.execute("insert into t values (1)")
c.commit()
print(c.execute("select * from t").fetchall())
for q in ["attach 'tests/sandbox/scoped/x.db' as s", "attach 'tests/sandbox/scoped/in/x.db' as s"]:
    try:
        c.execute(q)
        print("ok", q)
    except Exception as e:
        print("refused", q, e)
try:
    sqlite3.connect("tests/sandbox/scoped/secret.txt").execute("select 1")
    print("opened secret")
except Exception as e:
    print("refused secret", e)
