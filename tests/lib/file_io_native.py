# open(), os.listdir, os.walk and os.path's file tests, natively: CPython's modes, newline handling, reprs and
# errors ("[Errno 2] No such file or directory: 'x'"). Reads tests/lib/tree, writes one file in /tmp.
import os

root = os.path.join("tests", "lib", "tree")
out = "/tmp/brevi_file_io_native.txt"
with open(out, "w") as f:
    print(repr(f), f.mode, f.name, f.closed, f.readable(), f.writable())
    print(f.write("one\ntwo\r\nthree"))
    f.writelines(["\nfour", "\n"])
print(f.closed)
with open(out) as f:
    print(f.read().splitlines())
with open(out, newline="") as f:
    print(repr(f.read()))
with open(out, "rb") as f:
    print(repr(f), f.read(5), f.readline(), f.read())
for i, line in enumerate(open(out, encoding="utf-8")):
    print(i, repr(line))
print(open(out).readlines(), open(out).readline(2))
open(out, "a").write("five\n")
print(open(out).read().count("\n"), os.path.getsize(out))
with open(out, "wb") as f:
    f.write(b"\x00\x01bytes\n")
print(open(out, "rb").read())
for bad, err in [((out, "x"), FileExistsError), (("tests/lib/tree/missing.txt",), FileNotFoundError), ((root,), IsADirectoryError),
                 ((out, "rw"), ValueError), ((out, "q"), ValueError), ((out, "rb"), None)]:
    try:
        f = open(*bad, encoding="utf-8") if bad[-1] == "rb" else open(*bad)
        print("opened", bad)
    except Exception as e:
        print(type(e).__name__, e, isinstance(e, err) if err else "")
f = open(out)
f.close()
try:
    f.read()
except ValueError as e:
    print("ValueError", e)
try:
    open(out, "w").read()
except OSError as e:
    print("OSError", e)
try:
    open(os.path.join(root, "c", "blob.bin")).read()
except UnicodeDecodeError as e:
    print("UnicodeDecodeError", e)
print(repr(open(os.path.join(root, "a", "b", "y.txt")).read()))

print(sorted(os.listdir(root)))
for top, dirs, files in os.walk(root):
    dirs.sort()
    print(top, dirs, sorted(files))
# bottom-up order follows the order the file system lists each folder in, so sorted here
print(sorted((top, sorted(dirs), sorted(files)) for top, dirs, files in os.walk(root, topdown=False)))
a = os.path.join(root, "a")
print(os.path.exists(a), os.path.isdir(a), os.path.isfile(a), os.path.isfile(os.path.join(a, "x.py")), os.path.exists("nope"), os.path.islink(a), os.path.lexists(a))
print(os.path.getsize(os.path.join(a, "x.py")), list(os.walk("nope")))
for p in ["nope", os.path.join(a, "x.py")]:
    try:
        os.listdir(p)
    except OSError as e:
        print(type(e).__name__, e)
