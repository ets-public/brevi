# Path text from os.path and pathlib.PurePath runs natively (Brevi's own copies), not in CPython.
import os
from pathlib import PurePath

p = os.path.join("data", "in", "file.tar.gz")
print(p, os.path.basename(p), os.path.dirname(p), os.path.splitext(p))
print(os.path.normpath("a//b/../c"), os.path.isabs("/x"), os.sep)
q = PurePath("/srv/app/main.py")
print(q.name, q.suffix, q.stem, q.parent, q.parts)
