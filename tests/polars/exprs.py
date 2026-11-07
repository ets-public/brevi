# Expressions: arithmetic and casts, strings, lists, when/then, over, fold, rounding, statistics, Series building.
import polars as pl, random
import operator as o
s = pl.Series("a", [-7, 7, 0, None])
for name, r in [("mod", s % 3), ("fdiv", s // 2), ("fdiv0", s // 0), ("mod0", s % 0), ("div0", s / 0), ("pow", s ** 2), ("powf", s ** 0.5), ("neg", -s)]:
    print(name, r.dtype, r.to_list())
print(pl.select(pl.lit(1)).dtypes, pl.select(pl.lit(2**40)).dtypes, pl.select(pl.lit(1.5)).dtypes, pl.select(pl.lit("x")).dtypes, pl.select(pl.lit(None)).dtypes, pl.select(pl.lit(True)).dtypes)
i32 = pl.Series([1, 2], dtype=pl.Int32)
print((i32 + 1).dtype, (i32 + 1.5).dtype, (i32 * pl.Series([1, 2])).dtype, (i32 / 2).dtype)
print(pl.Series([2**62]) * 4)
print(pl.Series([1.5, -2.5]).cast(pl.Int64).to_list(), pl.Series(["1", "x"]).cast(pl.Int64, strict=False).to_list())
try:
    pl.Series(["1", "x"]).cast(pl.Int64)
except Exception as e:
    print(type(e).__name__, e)
print(pl.Series([0.5, 1.5, 2.5, -0.5]).round(0).to_list(), pl.Series([1.25, 1.35]).round(1).to_list())
print(pl.Series([3, 1, None, 2]).sort().to_list(), pl.Series([3, 1, None, 2]).sort(descending=True).to_list())
print((pl.Series([1, None]) == 1).to_list(), (pl.Series([1,2]) > pl.Series([2, 1])).to_list())
print(pl.Series(["a", "b"]) + "x")
print(pl.Series([1,2,3]).std(), pl.Series([1]).std(), pl.Series([1,2,3,4]).median(), pl.Series([1,2,3,4]).quantile(0.25))
print(pl.Series([1.0, float("nan"), 3.0]).max(), pl.Series([1.0, float("nan"), 3.0]).sum(), pl.Series([], dtype=pl.Int64).sum(), pl.Series([], dtype=pl.Int64).mean(), pl.Series([None], dtype=pl.Int64).max())
print(repr(pl.Int64), pl.List(pl.Int64), pl.Int64 == pl.Int64(), pl.Float64 == pl.Int64, pl.Utf8)
print(pl.Series(["a\nb", 'q"z', "tab\tx"]))
print(pl.DataFrame({"s": ["a\nb", "x"], "n": [1, 2]}))
print(pl.Series([1.0e-7, 123456789.0, 1234567.5, 0.1234567, 12.0000001, 99999.99999, 1e15, 2.5e-5]))
df = pl.DataFrame({
    "name": ["Ann", "bob", "Cara", None, "dave"],
    "age": [34, None, 29, 41, 22],
    "score": [88.5, 92.25, None, 70.0, 65.125],
    "tags": [["a", "b"], [], ["c"], None, ["a", "c", "d", "e"]],
    "team": ["x", "y", "x", "y", "x"],
})
print(df)
print(df.with_columns(
    pl.col("name").str.to_lowercase().alias("lower"),
    pl.col("name").fill_null("?").str.pad_start(6, "*").alias("pad"),
    pl.col("name").str.len_bytes().alias("nb"),
))
print(df.with_columns(pl.col("age").map_elements(lambda x: x * 2 if x % 2 else -x, return_dtype=pl.Int64).alias("a2")))
print(df.select(pl.col("tags").list.first(), pl.col("tags").list.join("-").alias("j"), pl.col("tags").list.contains("a").alias("has_a")))
print(df.explode("tags"))
print(df.with_columns(age_bucket=pl.when(pl.col("age") < 30).then(pl.lit("young")).when(pl.col("age") < 40).then(pl.lit("mid")).otherwise(pl.lit("old"))))
print(df.sort("age", nulls_last=True))
print(df.sort("score", descending=True))
print(df.select(pl.col("score").fill_null(strategy="forward"), pl.col("age").fill_null(pl.col("age").mean()).alias("age2")))
print(df.group_by("team", maintain_order=True).agg(pl.col("name"), pl.col("score").max(), pl.col("age").filter(pl.col("age") > 25).count().alias("over25")))
print(df.to_dict(as_series=False))
for key, g in df.group_by("team", maintain_order=True):
    print(key, g.height)
print(df.partition_by("team", as_dict=False)[0].shape)
print(df.select(pl.fold(acc=pl.lit(0), function=lambda a, b: a + b, exprs=[pl.col("age").fill_null(0), pl.lit(1)]).alias("f")))
print(df.select(pl.concat_str([pl.col("name"), pl.col("team")], separator="/").alias("cs")))
print(df.select(pl.col("score").std().alias("sd"), pl.col("score").var().alias("va"), pl.col("score").quantile(0.5).alias("q"), pl.col("age").sum().alias("s"), pl.col("age").product().alias("p")))
print(df.join(pl.DataFrame({"k": [1, 2]}), how="cross").shape)
a = pl.DataFrame({"k1": [1, 1, 2], "k2": ["a", "b", "a"], "v": [1, 2, 3]})
b = pl.DataFrame({"k1": [1, 2, 2], "k2": ["a", "a", "b"], "w": [10, 20, 30]})
print(a.join(b, on=["k1", "k2"], how="full", coalesce=True).sort("k1", "k2"))
print(a.join(b, left_on="k1", right_on="k1", how="left"))
s = pl.Series("x", [3, 1, None, 2, 3])
print(s.unique().sort(), s.sort(descending=True), s.is_null(), s.n_unique(), s.null_count())
print(s.to_frame(), s.alias("y").name, s.cast(pl.Utf8), s.fill_null(0).cum_sum())
print(s.value_counts(sort=True))
print(s.to_list(), s.len(), s.is_in([1, 2]).to_list(), (s * 2).to_list(), s.arg_max(), s.arg_min())
pl.Config.set_tbl_rows(4)
print(pl.DataFrame({"v": list(range(10))}))
pl.Config.set_tbl_rows(10)
with pl.Config(tbl_cols=3, fmt_str_lengths=5):
    print(df)
wide = pl.DataFrame({f"column_number_{i}": [i * 1000.5, i] for i in range(12)})
print(wide)
try:
    df.select(pl.col("nope"))
except pl.exceptions.ColumnNotFoundError as e:
    print("CNF", e)
try:
    df.with_columns(pl.col("name") + 1)
except Exception as e:
    print(type(e).__name__, e)
print(df.lazy().with_columns((pl.col("age") + 1).alias("next")).filter(pl.col("next").is_not_null()).select("name", "next").collect())
print(df.head(2).vstack(df.tail(1)))
print(df.drop("tags").write_csv())
print(df.select("name", "age").write_json())
print(df.schema, df.columns)
print(df.select(pl.col(pl.Int64)).columns, df.select(pl.col(pl.Float64, pl.Utf8)).columns)
print(df.with_columns(pl.col("age").clip(25, 35).alias("c")).get_column("c").to_list())
print(df.select(pl.col("age").is_between(25, 35)).to_series().to_list())
print(df.filter(pl.col("name").is_in(["Ann", "dave"])).height, df.filter(pl.col("name").str.contains("^[A-Z]")).height)
print(df.item(0, "age"), df[1, "name"], df[0:2, ["name", "age"]].shape, df[-1])
df=pl.DataFrame({'a':[1,2],'b':[3,None],'c':[0.5,1.5]})
print(df.select(pl.sum_horizontal(pl.all())))
print(df.select(pl.max_horizontal(pl.col("a","b")).alias("m"), pl.concat_str(pl.all(), separator="|").alias("s")))
print(df.with_columns(pl.mean_horizontal(pl.exclude("c"))))
print(df.select(pl.fold(acc=pl.lit(0), function=lambda a, b: a + b, exprs=[pl.col('a'), pl.col('b')])))
print(df.select(pl.fold(pl.lit(0), lambda a, b: a + b, pl.all()).alias('x')))
print(df.select(pl.reduce(lambda a, b: a + b, pl.all())))
print(df.select(pl.fold(pl.col('a'), lambda a, b: a * b, [pl.col('b')])))
print(df.select(pl.fold(pl.lit(0.0), lambda a, b: a + b, [pl.col('a'), pl.col('c')])))
print(df.select(pl.fold(pl.lit(0), lambda a, b: a + b, [pl.col('c')])).rows(), df.select(pl.fold(pl.lit(0), lambda a, b: a + b, [pl.col('c')])).dtypes)
print(df.select(pl.fold(pl.col('a'), lambda a, b: a + b, [pl.col('c')])).rows())
print(df.select(pl.fold(pl.lit(0), lambda a, b: a + b, [pl.col('c')], return_dtype=pl.Float64)).rows())
df=pl.DataFrame({'name':['a'],'n':[1],'zz':['q'], 'f':[1.0]})
for nm,f in [('add',o.add),('sub',o.sub),('mul',o.mul),('truediv',o.truediv),('floordiv',o.floordiv),('mod',o.mod),('pow',o.pow),('eq',o.eq),('ne',o.ne),('lt',o.lt),('ge',o.ge)]:
  for lhs in ['name','n']:
    try: r=df.select(f(pl.col(lhs),'zz')).rows(); print(nm,lhs,'OK',r)
    except Exception as x: print(nm, lhs, type(x).__name__, str(x).split(chr(10))[0])
  try: r=df.select(f('zz', pl.col('name'))).rows(); print(nm,'rev OK',r)
  except Exception as x: print(nm, 'rev', type(x).__name__, str(x).split(chr(10))[0])
for e in [pl.col('name')+1, pl.col('name')+pl.col('n'), pl.col('n')+pl.col('name'), pl.col('name')*2.5, pl.lit(1)+pl.col('name'), pl.col('name')+1.5, pl.col('f')-pl.col('name')]:
  try: df.select(e)
  except Exception as x: print(type(x).__name__, x)
s = pl.Series("x", [3.0, 1.0, 2.0, 2.0, 5.5])
print(s.entropy(), s.kurtosis(), s.skew(), s.dot(s), s.approx_n_unique())
print(s.mode(), s.rank(), s.arg_sort(), s.top_k(2), s.pct_change(), s.rolling_mean(2), s.rolling_sum(2), s.is_duplicated())
print(s.sqrt().round(3), s.log().round(4), s.clip(2, 3), s.cum_max(), s.unique_counts(), s.gather([0, 2]), s.gather_every(2))
print(s.replace({2.0: 20.0}), s.reverse(), s.shift(-1), s.diff(), s.interpolate())
print(s.mean(), s.std(), s.var(), s.median(), s.quantile(0.25), s.sum(), s.min(), s.max(), s.product(), s.n_unique(), s.first(), s.last())
print(s.is_sorted(), s.sort().is_sorted(), pl.Series([None, 1, 2]).is_sorted(), pl.Series([3, 1]).is_sorted(descending=True), pl.Series([1.0, float("nan")]).is_sorted())
print(pl.Series([0.0,1.0,2.0]).entropy(), pl.Series([1,2,3]).entropy(base=2), pl.Series([0.5]).entropy(), pl.Series([None, 1.0, 3.0]).entropy(), pl.Series([1.0,3.0]).entropy(normalize=False))
import random
random.seed(5)
big = pl.Series([random.gauss(0, 3) for _ in range(1000)])
print(round(big.skew(), 12), round(big.kurtosis(), 12), round(big.std(), 12))
print(pl.Series([1.0, 1.0, 1.0]).skew(), pl.Series([1.0, 2.0]).kurtosis(bias=False))
df = pl.DataFrame({"id": [1, 2, 3, 4, 5], "salary": [120000.0, 85000.5, None, 70000, 92000], "g": ["a", "b", "a", "b", "a"]})
print(df.select(pl.corr("id", "salary"), pl.cov("id", "salary").alias("cv"), pl.corr("id", "salary", method="spearman").alias("sp")))
print(df.group_by("g", maintain_order=True).agg(pl.corr("id", "salary").alias("c"), pl.cov("id", "salary").alias("v")))
random.seed(3)
big = pl.DataFrame({"x": [random.random() for _ in range(700)], "y": [random.random() for _ in range(700)]})
print(big.select(pl.corr("x", "y").round(12), pl.cov("x", "y").round(12).alias("c")).rows())
print(pl.DataFrame({"x": [1.0, 1.0], "y": [2.0, 3.0]}).select(pl.corr("x", "y")).rows(), pl.DataFrame({"x": [1.0], "y": [2.0]}).select(pl.cov("x", "y")).rows())
print(pl.select(pl.lit(1) + 1).rows(), pl.select(x=pl.lit(2)).rows())
print(pl.col("a") + pl.len(), pl.len())
for v in [[1, True], [None, 2.5, 1], [1, 2.5], [1.5, 'a'], ['a', 1], [True, 1], [1, 2**70], [1.0, True], [None, None], [2**63 - 1, -2**63]]:
    try:
        s = pl.Series('s', v); print(v, s.dtype, s.to_list())
    except Exception as e:
        print(v, type(e).__name__, repr(str(e)))
try:
    pl.DataFrame({'a': [1, 2.5]})
except Exception as e:
    print(repr(str(e)))
random.seed(1)
vals=[round(random.uniform(-100,100), random.randint(1,6)) for _ in range(3000)]+[random.uniform(-1e6,1e6) for _ in range(500)]+[1e300,1e-300,123456789.125, 5e-324, 1e20+0.5,2.675,0.49999999999999994,-0.5,0.5,-2.5]
for d in [0,1,2,3,7,300,310,320,330]:
    print(d, pl.Series(vals).round(d).to_list()[-12:], sum(pl.Series(vals).round(d).to_list()))
    print(d, pl.Series(vals).round(d, mode='half_away_from_zero').to_list()[-12:], hash(tuple(pl.Series(vals).round(d, mode='half_away_from_zero').to_list())))
    print(hash(tuple(pl.Series(vals).round(d).to_list())))
