# os for sandboxed builds (src/dynpy/mod.rs): os.path and the path constants, nothing that reaches files,
# processes or the environment.
import posixpath as path

sep = "/"
curdir = "."
pardir = ".."
extsep = "."
altsep = None
linesep = "\n"
name = "posix"
