import sqlite3
import hashlib
import zlib
import statistics
from datetime import date, timedelta
con = sqlite3.connect(":memory:")
con.execute("create table s (name text, score real)")
con.executemany("insert into s values (?, ?)", [("a", 1.5), ("b", 2.5), ("c", 4.0)])
for row in con.execute("select name, score from s order by score desc"):
    print(row)
print(hashlib.sha256("brevi".encode()).hexdigest())
data = zlib.compress("hello hello hello hello".encode())
print(len(data), zlib.decompress(data))
print(statistics.median([3, 1, 4, 1, 5]))
d = date(2026, 11, 3) + timedelta(days=30)
print(d.strftime("%A %d %B %Y"))
