import csv, io
r = csv.DictReader(io.StringIO("a,b\n1,2\n"))
print(r.fieldnames)
print(list(r))
print(r.line_num, r.restkey, r.restval)
r = csv.DictReader(io.StringIO("a,b\n1\n2,3,4\n\n5,6\n"), restkey="extra", restval="MISS")
print(r.fieldnames)
print(list(r))
print(r.line_num)
r = csv.DictReader(io.StringIO("a,b\n1,2\n"))
print(r.dialect, r.reader.line_num)
print(next(r), r.reader.line_num)
r = csv.DictReader(io.StringIO("a,b\n1,2\n"), restkey="r", restval="v")
r.fieldnames = ["x", "y"]
print(r.fieldnames)
print(list(r))
r = csv.DictReader(io.StringIO("\n\na,b\n1,2\n"))
print(r.fieldnames)
print(list(r))
r = csv.DictReader(io.StringIO("a,b\n"))
print(r.fieldnames)
print(list(r))
r = csv.DictReader(io.StringIO("a,b\n1,2\n"), fieldnames=["p", "q"])
print(r.fieldnames)
print(list(r))
# line_num counts physical source lines, including the lines a quoted field spans
r = csv.DictReader(io.StringIO('a,b\n1,"x\ny"\n3,4\n'))
for row in r:
    print(r.line_num, r.reader.line_num)
# csv.reader itself is the reader object with .line_num and .dialect
r = csv.reader(io.StringIO('a,b\n1,"x\ny"\n3,4\n'))
print(r.line_num, r.dialect.delimiter, r.dialect.quotechar, repr(r.dialect.lineterminator))
print(next(r), r.line_num)
print(next(r), r.line_num)
# named dialects excel, excel-tab and unix; unknown names raise csv.Error
for name in ("excel", "excel-tab", "unix"):
    rr = csv.reader(io.StringIO("a\tb\n1\t2\n"), dialect=name)
    print(name, list(rr))
print(list(csv.reader(io.StringIO("a\tb\n1\t2\n"), "excel-tab")))
try:
    csv.reader(io.StringIO("a\n"), dialect="nope")
except csv.Error as e:
    print("Error", e)
try:
    csv.DictReader(io.StringIO("a\n"), dialect="nope")
except csv.Error as e:
    print("Error", e)
