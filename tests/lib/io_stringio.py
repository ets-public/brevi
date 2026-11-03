import csv
import io
from io import StringIO

buf = io.StringIO()
w = csv.writer(buf, lineterminator="\n")
w.writerow(["id", "name"])
w.writerows([[1, "a,b"], [2, 'say "hi"']])
print(buf.getvalue(), end="")
print(buf.tell(), len(buf.getvalue()))

r = csv.DictReader(io.StringIO("a,b\n1,2\n3,4\n"))
print([dict(x) for x in r])

s = StringIO("line one\nline two\nlast")
print(repr(s.readline()), repr(s.read(4)), s.tell())
print([l for l in s])
s.seek(0)
print(s.readlines())
s.seek(0)
s.write("LINE")
print(s.getvalue())
s.truncate(4)
print(repr(s.getvalue()), s.write("!"), s.getvalue())

out = StringIO()
print("hello", 42, file=out)
print("x", "y", sep="-", end="?\n", file=out)
with StringIO("ab\ncd\n") as f:
    print(sum(1 for _ in f))
print(repr(out.getvalue()), f.closed)
try:
    f.read()
except ValueError as e:
    print("ValueError:", e)
try:
    out.write(5)
except TypeError as e:
    print("TypeError:", e)
