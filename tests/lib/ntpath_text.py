# ntpath (os.path on Windows) runs natively on every platform, with CPython's answers for drive letters, UNC
# shares, both slashes and relative drive paths; posixpath's splitroot/splitdrive likewise.
import ntpath
import posixpath

paths = ["C:\\Users\\a\\file.txt", "C:/Users/a/file.tar.gz", "\\\\server\\share\\dir\\x.py", "\\\\server\\share",
         "\\\\?\\UNC\\srv\\sh\\f", "\\\\.\\device\\x", "C:", "C:rel\\p", "\\rooted\\p", "rel/dir\\mixed.txt",
         "..\\up\\.\\x", "C:\\a\\..\\..\\b", "\\..\\x", ".hidden", "a.b.c", "dir\\", "", "c:\\", "//x/y/z"]
for p in paths:
    print(repr(p), ntpath.splitroot(p), ntpath.splitdrive(p), ntpath.split(p), ntpath.splitext(p))
    print("  ", ntpath.basename(p), repr(ntpath.dirname(p)), ntpath.isabs(p), repr(ntpath.normpath(p)), repr(ntpath.normcase(p)))
for a, b in [("C:\\a", "b"), ("C:\\a", "D:\\x"), ("C:\\a", "\\x"), ("a", "b/"), ("\\\\srv\\sh", "x"), ("C:", "x"),
             ("c:\\a", "C:b"), ("c:\\a", "d:b"), ("", "x"), ("a\\", "\\\\srv\\sh\\y"), ("C:a", "C:\\b")]:
    print(repr(a), repr(b), repr(ntpath.join(a, b)))
print(ntpath.join("C:\\", "a", "b", "c.txt"), ntpath.commonprefix(["C:\\a\\b", "C:\\a\\c"]), ntpath.sep, ntpath.altsep)
for p in ["/a/b", "//a/b", "///a", "a/b", ""]:
    print(repr(p), posixpath.splitroot(p), posixpath.splitdrive(p))
