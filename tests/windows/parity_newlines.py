# Text-mode newlines on Windows: print and write translate "\n" to "\r\n" on stdout and in files opened in
# text mode, reading translates back, and newline="" keeps the bytes as they are.
import os
import sys
import tempfile

print("line one")
sys.stdout.write("line two\n")
print("a", "b", sep="\n")

d = tempfile.mkdtemp()
p = os.path.join(d, "t.txt")
with open(p, "w") as f:
    f.write("x\ny\n")
with open(p, "rb") as f:
    print(f.read())
with open(p, "wb") as f:
    f.write(b"crlf\r\nlf\ncr\rend")
with open(p) as f:
    print(repr(f.read()))
with open(p, newline="") as f:
    print(repr(f.read()))
with open(p, "w", newline="\n") as f:
    f.write("kept\n")
with open(p, "rb") as f:
    print(f.read())
os.remove(p)
os.rmdir(d)
