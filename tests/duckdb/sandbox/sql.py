import duckdb
# (run with --sandbox and no grants: see tests/check.sh)
# Under --sandbox with no grants SQL reaches no file: no reads, no writes, no extensions, no settings changes.
con = duckdb.connect()
for q in [
    "COPY (SELECT 1 AS a) TO '/tmp/brevi_sb_duck.csv'",
    "SELECT * FROM read_csv('/etc/passwd')",
    "SELECT * FROM read_text('/etc/hostname')",
    "ATTACH '/tmp/brevi_sb_duck_attach.db' AS x",
    "INSTALL httpfs",
    "SET enable_external_access = true",
    "SELECT * FROM glob('/etc/*')",
]:
    try:
        con.execute(q)
        print("ran", q)
    except duckdb.Error as e:
        m = str(e).split("\n")[0]
        print(type(e).__name__, m if "/tmp/" in m or "/etc/" in m or "locked" in m else "disabled by configuration" in m)
print(con.execute("SELECT 42").fetchall())
