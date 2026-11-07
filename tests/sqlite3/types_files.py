import datetime
import decimal
import json
import sqlite3

path = "/tmp/brevi_sqlite3_test.db"
with sqlite3.connect(path) as old:
    old.execute("drop table if exists ev")
old.close()
con = sqlite3.connect(path, detect_types=sqlite3.PARSE_DECLTYPES | sqlite3.PARSE_COLNAMES)
con.execute("create table ev(id integer primary key autoincrement, day date, at timestamp, meta json, price real, n int)")


class Point:
    def __init__(self, x, y):
        self.x, self.y = x, y

    def __conform__(self, protocol):
        if protocol is sqlite3.PrepareProtocol:
            return f"{self.x};{self.y}"

    def __repr__(self):
        return f"Point({self.x}, {self.y})"


sqlite3.register_adapter(dict, json.dumps)
sqlite3.register_adapter(decimal.Decimal, str)
sqlite3.register_converter("JSON", json.loads)
sqlite3.register_converter("point", lambda b: Point(*map(float, b.split(b";"))))
rows = [
    (datetime.date(2026, 11, 7), datetime.datetime(2026, 11, 7, 12, 30, 15), {"k": [1, 2]}, 9.5, 3),
    (datetime.date(1999, 1, 2), datetime.datetime(1999, 1, 2, 3, 4, 5, 678000), {"k": None}, decimal.Decimal("1.25"), None),
]
con.executemany("insert into ev(day, at, meta, price, n) values (?, ?, ?, ?, ?)", rows)
con.commit()
for r in con.execute("select * from ev"):
    print(r)
print(con.execute("select at as \"t [timestamp]\" from ev").fetchall())
print(con.execute("select ? as \"p [point]\"", (Point(1, 2),)).fetchone())
print(con.execute("select '5;6' as \"p [point]\", 1 as \"n [unknown]\"").description)
print(con.execute("select * from sqlite_sequence").fetchall())
con.close()
con = sqlite3.connect(path)
print(con.execute("select day, at, meta from ev").fetchall())
print([r[1] for r in con.execute("pragma table_info(ev)")])
con.row_factory = lambda cur, row: {col[0]: v for col, v in zip(cur.description, row)}
print(con.execute("select id, n from ev order by id").fetchall())
con.row_factory = sqlite3.Row
for r in con.execute("select id, price from ev"):
    print(r.keys(), r["ID"], r["price"], tuple(r))
cur = con.cursor()
cur.row_factory = None
print(cur.execute("select count(*) from ev").fetchone())
con.close()
uri = "file:" + path + "?mode=ro"
ro = sqlite3.connect(uri, uri=True)
print(ro.execute("select count(*) from ev").fetchone())
try:
    ro.execute("delete from ev")
except sqlite3.OperationalError as e:
    print(type(e).__name__, e, e.sqlite_errorname)
ro.close()
with sqlite3.connect(path) as c2:
    c2.execute("insert into ev(n) values (42)")
c3 = sqlite3.connect(path, isolation_level=None)
print(c3.execute("select n from ev order by id desc limit 1").fetchone(), c3.in_transaction)
print("\n".join(c3.iterdump()))
c3.close()
mem = sqlite3.connect("")
mem.execute("create table x(a)")
print(mem.execute("select count(*) from x").fetchone())
print("done")
