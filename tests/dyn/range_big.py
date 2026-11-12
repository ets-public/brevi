import sys
M = sys.maxsize
r = range(M-5, M+5)
print(r, len(r), list(r)[:3], r[-1], r[0], (M+2) in r, r.index(M+1), r.count(M+100))
print(list(range(-M-5, -M+5)))
print(list(range(10, 10+3*(M+5), M+5)))
r2 = range(M-15, M+100, 3)
print(list(r2)[:4], list(reversed(r2))[:3], r2.start, r2.stop, r2.step, bool(r2))
print(isinstance(r2, range), type(r2).__name__, type(iter(r2)).__name__)
print(range(M+1, M+1) == range(0), hash(range(M+1, M+1)) == hash(range(0)), r == range(M-5, M+5), hash(r) == hash(range(M-5, M+5)))
print(range(0, M+10, 1) == range(0, M+10))
try: len(range(0, 2*M*4))
except OverflowError as e: print("OE", e)
try: r[100]
except IndexError as e: print("IE", e)
try: r.index(3)
except ValueError as e: print("VE", e)
print(repr(range(M, M+10, -1)), list(range(M, M+10, -1)))
print(hash(range(0)), hash((0,None,None)), hash(range(M+1,M+1)), hash(range(5,6)), hash((1,5,None)), hash(range(1,9,2)), hash((4,1,2)))
m = -M - 1
print(list(reversed(range(m, m+3))), list(range(M-2, M)), list(range(M-2, M, 5)), list(reversed(range(M-3, M, 2))))
print(list(range(m+2, m, -1)), list(reversed(range(m+2, m, -1))), len(range(m, M)) if False else 0)
print(list(range(M-1, m, -M)), list(reversed(range(M-1, m, -M))))
