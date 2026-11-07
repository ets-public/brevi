import duckdb
c=duckdb.connect()
print(c.sql(r"select columns(*)::varchar from (select 1 a, 2 a, [1,2] l, {'x':'y'} s, 1.5::float f, 1e100 d, '\xff'::blob b, interval 1 month i, timestamp '2020-01-01' t, 1.10::decimal(4,2) dd, 0.1::float ff, true bb, ['a', null] ls, map{'k': [1]} m, date '2020-01-02' dt, time '01:02:03.5' tm, 0.1 + 0.2 sm, 1/3 th)").fetchall())
for q in ['select ?, ?', 'select $a, $b']:
  for p in [None, [], [1], [1,2,3], {'a':1}, {'b':1,'a':2,'c':3}]:
    try: print(q, p, c.execute(q, p).fetchall())
    except Exception as e: print(q, p, type(e).__name__, e)
