# Brevi's polars reads and writes files only with the caller's grants: under --sandbox this needs
# --allow read,write (the build is refused without them).
import polars as pl

path = "/tmp/brevi_polars_sandbox.csv"
pl.DataFrame({"a": [1, 2], "b": ["x", None]}).write_csv(path)
print(pl.read_csv(path))
