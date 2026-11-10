def tryit(f, *a, **k):
    try:
        print(repr(f(*a, **k)))
    except Exception as e:
        print(type(e).__name__, e)


print("abcab".translate(str.maketrans("ab", "xy", "c")))
print("abc".maketrans("ab", "xy", "c"))
print(str.maketrans("ab", "xy"))
print(str.maketrans("", "", ""))
print(str.maketrans({"a": "x", "c": None}))
print(str.maketrans({1: 2}))
print(str.maketrans({}))
tryit(str.maketrans)
tryit(str.maketrans, 1)
tryit(str.maketrans, {"ab": 1})
tryit(str.maketrans, "ab", "x")
tryit(str.maketrans, "ab", "x", "yz")
tryit(str.maketrans, "a", 2)
tryit(str.maketrans, "a", "b", 3)
tryit(str.maketrans, "a", "b", "c", "d")
tryit(str.maketrans, "a", "b", z=1)
