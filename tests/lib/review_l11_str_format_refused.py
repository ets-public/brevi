# Nested replacement fields in a format spec share the automatic numbering with
# the outer format string (CPython's auto_arg_index threads through the spec).
print("{0}-{1}".format("A", "B"), "{:>{}}".format("abc", 7))
cases = [
    lambda: "{:{}}".format("abc", 7),
    lambda: "{:>{}}".format("abc", 7),
    lambda: "{} {:>{}}".format("a", "abc", 7),
    lambda: "{2:>{0}}".format("x", "abc", 7),
    lambda: "{2:>{1}}".format("x", "abc", 7),
    lambda: "{1:{}}".format("x", 7),
    lambda: "{} {0}".format(1, 2),
    lambda: "{a:{b}}".format_map({"a": "abc", "b": 7}),
]
for c in cases:
    try:
        print(repr(c()))
    except Exception as e:
        print(type(e).__name__ + ": " + str(e))
