# pathlib.Path reads and writes files: refused without allow native.
from pathlib import Path
print(Path("tests/sandbox/data.txt").read_text())
