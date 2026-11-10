import sys

# Typed build: print with sep=/end= and stderr writes must use CPython's CRLF on Windows too.
n = 3
print("a", n, sep="-")
print("no newline", end="")
print(" then one", end="\n")
print("two\nlines")
print(1, 2, 3, sep=", ", end="!\n")
print("to stderr", file=sys.stderr)
print("done")
