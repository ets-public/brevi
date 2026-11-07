# Parquet is not in Brevi's polars: under --sandbox the build is refused (CPython's polars would need
# --allow-py polars); outside it the program runs with CPython's polars.
import polars as pl

print(pl.read_parquet("data.parquet"))
