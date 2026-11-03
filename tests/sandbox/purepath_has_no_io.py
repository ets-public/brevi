# Brevi's own PurePath has no file access at all, so a method that would read a file does not exist.
from pathlib import PurePath
p = PurePath("tests/sandbox/data.txt")
print(p.name)
print(p.read_text())
