import sqlite3
print(sqlite3.sqlite_version)
con = sqlite3.connect(":memory:")
print(con.execute("select sqlite_version()").fetchone()[0])
