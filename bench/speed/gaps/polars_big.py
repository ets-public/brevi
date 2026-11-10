# Filter and group a 20,000-row frame (the polars large-frame speed gap).
import sys
import time

import polars as pl

n = int(sys.argv[1]) if len(sys.argv) > 1 else 20000
reps = 10
cats = ["a", "b", "c", "d", "e"]
df = pl.DataFrame({"id": list(range(n)), "cat": [cats[i % 5] for i in range(n)],
                   "x": [(i * 37 % 1000) / 10 for i in range(n)], "y": [i % 13 for i in range(n)]})
t = time.perf_counter()
for _ in range(reps):
    out = (df.filter(pl.col("x") > 20.0)
             .with_columns((pl.col("x") * pl.col("y")).alias("xy"))
             .group_by("cat", maintain_order=True)
             .agg(pl.col("xy").sum().round(2).alias("s"), pl.col("id").count().alias("n"), pl.col("x").mean().alias("m"))
             .sort("cat"))
print(out)
print("time %.3f" % (time.perf_counter() - t), file=sys.stderr)
