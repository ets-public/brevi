# The platform as a program sees it on Windows: sys.platform, os.name, os.sep and friends, and os.path working
# on Windows paths (drive letters, backslashes and forward slashes, UNC shares) the way CPython's ntpath does.
import os
import sys

print(sys.platform, os.name, repr(os.sep), repr(os.altsep), repr(os.linesep))
paths = ["C:\\Users\\a\\file.txt", "C:/Users/a/file.tar.gz", "\\\\server\\share\\dir\\x.py", "C:", "C:rel\\p",
         "\\rooted\\p", "rel/dir\\mixed.txt", "..\\up\\.\\x", "C:\\a\\..\\..\\b", ".hidden", "dir\\", ""]
for p in paths:
    print(repr(p), os.path.basename(p), repr(os.path.dirname(p)), os.path.split(p), os.path.splitext(p))
    print("  ", os.path.isabs(p), repr(os.path.normpath(p)), repr(os.path.normcase(p)))
print(os.path.join("C:\\a", "b", "c.txt"), os.path.join("C:\\a", "D:\\x"), os.path.join("C:\\a", "\\x"))
print(os.path.join("a", "b/"), os.path.join("\\\\srv\\sh", "x"), os.path.join("C:", "x"), os.path.join("c:\\a", "C:b"))
print(os.path.commonprefix(["C:\\a\\b", "C:\\a\\c"]))
print(os.path.basename(sys.argv[0]).lower().replace(".exe", "").replace(".py", ""))
