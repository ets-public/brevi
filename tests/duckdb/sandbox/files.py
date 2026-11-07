import duckdb

# A database file needs the read grant, and the write grant unless it is opened read_only: under --sandbox the
# build refuses this program without them. Memory databases need nothing.
mem = duckdb.connect()
ro = duckdb.connect("/tmp/brevi_sb_duck_ro.duckdb", read_only=True)
rw = duckdb.connect("/tmp/brevi_sb_duck_rw.duckdb")
