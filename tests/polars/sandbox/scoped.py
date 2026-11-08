# polars under --allow-read / --allow-write DIR: files only inside the granted folders.
import polars as pl

df = pl.read_csv("tests/sandbox/scoped/in/d.csv")
print(df.shape)
for p in ["tests/sandbox/scoped/secret.txt", "tests/sandbox/scoped/in/link.txt"]:
    try:
        pl.read_csv(p)
        print("read", p)
    except Exception as e:
        print("refused", e)
df.write_csv("tests/sandbox/scoped/out/o.csv")
print("wrote out/o.csv")
try:
    df.write_csv("tests/sandbox/scoped/in/o.csv")
    print("wrote in/o.csv")
except Exception as e:
    print("refused", e)
