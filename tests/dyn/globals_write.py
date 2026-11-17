# Writing through globals() is refused in native builds (it would bypass direct calls
# and module slots), so brevi run runs the file with python3.
import sys
LEVEL = 1


def bump():
    globals()["LEVEL"] = 2


bump()
print(LEVEL)
