# A program that writes a file is not run a second time: a regex built at run time with a back-reference
# stops it with an error, never a wrong match.
import re
import sys

with open("/dev/null", "w") as f:
    f.write("x")
print("before")
p = "(" + sys.argv[0][:0] + "a)" + "\\1"
m = re.search(p, "xaay")
print(m.group(0) if m else None)
