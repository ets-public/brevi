import sys, csv

# csv rows to stdout (CRLF terminator) and print(sep=/end=): CPython translates every LF on Windows.
n = 3
print("a", n, sep="-")
print("no newline", end="")
print(" then one", end="\n")
print(1, 2, 3, sep=", ", end="!\n")
print("to stderr", file=sys.stderr)
w = csv.writer(sys.stdout)
w.writerow([1, "a,b", 3])
print("done")
