# os.path, os.sep and pathlib.PurePath only work on path text: they run without allow native (Brevi's own copy,
# no CPython). Every other use of os and pathlib still needs allow native (see os_*.py).
import os
import os.path
from os.path import join as j, splitext
from pathlib import PurePath, PurePosixPath

p = os.path.join("data", "runs", "r1.json")
print(p, os.path.basename(p), os.path.dirname(p), splitext(p), os.path.split(p))
print(os.path.normpath("a//b/./c/../d/"), os.path.isabs(p), j("/x", "y"), os.sep)
q = PurePath("/srv") / "app" / "main.tar.gz"
print(q, repr(q), q.name, q.stem, q.suffix, q.suffixes, q.parent, q.parts)
print(q.with_suffix(".zip"), q.relative_to("/srv"), PurePosixPath("a", "b") == PurePath("a/b"))
