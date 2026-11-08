# Writes 3 MB to stderr where the caller allows 1 MB written: stdout and stderr do not count.
import sys

for i in range(3000):
    sys.stderr.write("x" * 999 + "\n")
sys.stderr.write("end\n")
print("done")
