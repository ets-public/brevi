print(type(iter(bytearray())).__name__, type(iter(memoryview(b"a"))).__name__, type(iter(bytearray())) is type(iter(bytearray(b"x"))))
for v, s in [(-0.0, "z.1f"), (-0.0001, "z.1f"), (-0.0, "+z.0f"), (-0.0, "z.1e"), (-1e-9, "z.2g"), (-0.0001, "z.0%"), (-0.0, "z>8.1f")]:
    print(repr(format(v, s)))
for v, s in [(1, "z"), ("a", "z")]:
    try:
        format(v, s)
    except ValueError as e:
        print(e)
