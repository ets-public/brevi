# INSERT/UPDATE ... RETURNING through every execute path (first run, the repeated-statement fast path, a
# cursor's execute): the cursor has a description, and converters apply to the returned columns.
import sqlite3

sqlite3.register_converter("pt", lambda b: ("PT", b))
c = sqlite3.connect(":memory:", detect_types=sqlite3.PARSE_DECLTYPES)
c.execute("create table t(id integer primary key, p pt)")
q = "insert into t(p) values (?) returning id, p"
for i in range(3):
    cur = c.execute(q, ("x%d" % i,))
    print(cur.description, cur.fetchone(), cur.rowcount, cur.lastrowid)
cur = c.cursor()
for i in range(2):
    cur.execute(q, ("c%d" % i,))
    print(cur.description, cur.fetchall(), cur.rowcount)
q2 = "update t set p = ? where id = ? returning p"
for v, k in (("y", 1), ("z", 2), ("w", 99)):
    cur = c.execute(q2, (v, k))
    print(cur.description, cur.fetchall(), cur.rowcount)

c2 = sqlite3.connect(":memory:")
c2.execute("create table u(a)")
s = "insert into u values (?) returning a, typeof(a)"
for v in (True, 1.5, None, "s", 7):
    cur = c2.execute(s, (v,))
    print([d[0] for d in cur.description], cur.fetchone())
cur = c2.execute("delete from u where a = ? returning a", (7,))
print([d[0] for d in cur.description], cur.fetchall(), cur.rowcount)
cur = c2.execute("insert into u values (?)", (8,))
print(cur.description, cur.rowcount, cur.lastrowid)
