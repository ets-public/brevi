import duckdb

# A database file: tables persist across connections, cursors share the database, read_only refuses writes.
path = "/tmp/brevi_duckdb_test.duckdb"
con = duckdb.connect(path)
con.execute("DROP TABLE IF EXISTS items")
con.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name VARCHAR, price DECIMAL(8, 2))")
con.executemany("INSERT INTO items VALUES (?, ?, ?)", [(1, "pen", "1.50"), (2, "book", "12.99"), (3, "lamp", "30.00")])
print(con.rowcount)
cur = con.cursor()
print(cur.execute("SELECT name, price FROM items ORDER BY id").fetchall())
cur.close()
try:
    con.execute("INSERT INTO items VALUES (1, 'dup', 0)")
except duckdb.ConstraintException as e:
    print("constraint:", str(e).split("\n")[0])
con.close()

con = duckdb.connect(path, read_only=True)
print(con.sql("SELECT count(*) AS n, sum(price) AS total FROM items"))
try:
    con.execute("DELETE FROM items")
except duckdb.Error as e:
    print(type(e).__name__, str(e).split("\n")[0])
con.close()

with duckdb.connect(path) as con:
    con.execute("UPDATE items SET price = price * 2 WHERE id = ?", [1])
    print(con.table("items").order("id").fetchall())
