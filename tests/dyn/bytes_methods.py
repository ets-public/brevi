class C:
    def __rmul__(self, o): return 'rmul'
print('x' * C(), [1] * C(), (1,) * C())
try: 'x' * b'y'
except TypeError as e: print(e)
try: 'x' * 1.5
except TypeError as e: print(e)
print(b'x' * C(), bytearray(b'x') * C())
for a, b in [(b'x', b'y'), (b'x', 1.5), (bytearray(b'x'), 'a'), ('x', b'y'), ([1], 1.5)]:
    try: a * b
    except TypeError as e: print(e)
print(b'ab'.rpartition(b'abc'), b''.rpartition(b'0a'), bytearray(b'abc').partition(b'b'), bytearray.maketrans(b'a', b'b')[97:98])
