# Writes a file: a second run would write it again, so run --verify runs it once, unverified.
import os, tempfile
p = os.path.join(tempfile.gettempdir(), "brevi-verify-writes.txt")
with open(p, "w") as f:
    f.write("x")
print("wrote")
