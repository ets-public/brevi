# An import after `;` is read too (not only at the start of a line): json is harmless, but the check cannot tell
# what a hidden import is without reading it, so it is refused, with the fix.
x = 1; import json
print(json.dumps(x))
