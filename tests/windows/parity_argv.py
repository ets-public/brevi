# sys.argv on Windows: the arguments come from one command line that the C runtime splits (quotes, backslashes,
# empty and non-ASCII arguments). check.ps1 runs this under CPython and as a built .exe with the same arguments.
# argv[0] is the program: the script for CPython, the .exe for a build, so only its stem is printed.
import os
import sys

print(os.path.splitext(os.path.basename(sys.argv[0]))[0])
print(len(sys.argv))
for a in sys.argv[1:]:
    print(repr(a))
