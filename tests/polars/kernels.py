# Edge cases of the native column kernels (_brevi_col): nulls, NaN, -0.0, mixed int/float comparisons,
# integer wrapping, bool arithmetic, sums over more than 128 values, grouping keys.
import polars as pl

n = 300
df = pl.DataFrame({
    "i": [(k * 7919) % 101 - 50 if k % 11 else None for k in range(n)],
    "f": [((k * 37) % 1000) / 7 if k % 13 else (float("nan") if k % 2 else None) for k in range(n)],
    "z": [0.0 if k % 2 else -0.0 for k in range(n)],
    "b": [k % 3 == 0 for k in range(n)],
    "s": ["s%d" % (k % 4) if k % 9 else None for k in range(n)],
    "u": pl.Series([k * 3 for k in range(n)], dtype=pl.UInt8) if False else [k % 200 for k in range(n)],
})
print(df.select(pl.col("f").sum().alias("fs"), pl.col("f").mean().alias("fm"), pl.col("i").sum(), pl.col("z").sum()))
for op in ["==", "!=", "<", "<=", ">", ">="]:
    e = {"==": pl.col("f") == 50, "!=": pl.col("f") != 50, "<": pl.col("f") < 50, "<=": pl.col("i") <= 3,
         ">": pl.col("i") > pl.col("f"), ">=": pl.col("s") >= "s2"}[op]
    print(op, df.select(e.alias("r"))["r"].to_list()[:40])
print(df.select((pl.col("f") == pl.col("f")).alias("r"))["r"].to_list()[:30])
print(df.select(pl.col("f").eq_missing(pl.col("f")).alias("r"))["r"].to_list()[:30])
print(df.select(pl.col("i").ne_missing(3).alias("r"))["r"].to_list()[:30])
print(df.select((pl.col("i") * pl.col("f")).alias("a"), (pl.col("i") - 2.5).alias("b"), (pl.col("f") + pl.col("z")).alias("c")).head(20))
print(df.select((pl.col("i") * pl.col("i") * pl.col("i")).alias("a"), (pl.col("b") + pl.col("b")).alias("bb"), (pl.col("b") * pl.col("i")).alias("bi")).head(15))
w = pl.DataFrame({"a": pl.Series([100, 120, -128, 127], dtype=pl.Int8), "u": pl.Series([200, 250, 3, 0], dtype=pl.UInt8),
                  "big": [2**62, -2**62, 2**63 - 1, 5]})
print(w.select((pl.col("a") * 3).alias("a3"), (pl.col("a") + pl.col("a")).alias("aa"), (pl.col("u") - 10).alias("u10"),
               (pl.col("u") * pl.col("u")).alias("uu"), (pl.col("big") * 4).alias("b4"), (pl.col("big") + pl.col("big")).alias("bb")))
print(df.filter(pl.col("f") > 70).height, df.filter(pl.col("b")).select(pl.col("i").sum()))
for key in ["z", "f", "s", "b", "i"]:
    g = df.group_by(key, maintain_order=True).agg(pl.len(), pl.col("u").sum().alias("us"), pl.col("f").max().alias("fmax"))
    print(g.head(8))
print(df.sort("f").head(5), df.sort("s", "i").tail(5))
