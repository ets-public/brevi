def t(f):
    try:
        print(f())
    except Exception as e:
        print(type(e).__name__, e)
m = memoryview(b'abc')
print(m[0], m[-1], m[1:].tobytes(), len(m), m.tolist(), m.readonly, m.format, m.itemsize, m.ndim, m.shape, m.strides, m.nbytes, m.obj, m == b'abc', b'abc' == m, m == memoryview(b'abc'), hash(m) == hash(b'abc'), list(m), bytes(m), m.hex(), m[::2].tobytes(), m[::-1].tolist(), m.c_contiguous, m.contiguous)
def w(): m[0] = 1
t(w)
x = bytearray(b'abc'); v = memoryview(x); v[0] = 65; print(x, v.readonly)
v[0:2] = b'xy'; print(x)
def w2(): v[0:2] = b'x'
t(w2)
def w3(): v[0] = 256
t(w3)
t(lambda: v[5])
t(lambda: hash(v))
r = memoryview(b'abc'); r.release()
t(lambda: r[0]); t(lambda: len(r)); print(repr(r).startswith('<released memory at 0x'))
t(lambda: memoryview('abc')); t(lambda: memoryview(5))
y = bytearray(b'ab'); mv = memoryview(y)
t(lambda: y.append(1))
mv.release(); y.append(1); print(y)
with memoryview(y) as m2:
    print(m2[0])
    t(lambda: y.append(2))
y.append(3); print(y)
t(lambda: m < m)
print(m.tobytes('A'), m.toreadonly().readonly)
t(lambda: m['a']); t(lambda: m[1.5])
q = memoryview(b'abcd'); print(q[1:3][::-1].tobytes(), q[1:3].obj is q.obj, q[True])
print(memoryview(memoryview(b'ab')).tobytes(), b'x'.join([memoryview(b'a'), b'b']), b'a' + memoryview(b'b'), bytearray(memoryview(b'z')))
z = bytearray(b'hello'); zv = memoryview(z)[1:4]; z[2] = ord('L'); print(zv.tobytes(), list(zv), zv[1], isinstance(zv, memoryview), type(zv).__name__)
import re
print(re.split(b':', memoryview(b'a:b')), re.search(b'(b)', memoryview(b'abc')).group(1), re.findall(b'.', memoryview(b'xy')))
print(repr(re.search(br'(.+)(.*?)\1', memoryview(b'[abracadabra]'))))
mm = re.match(b'a', memoryview(b'ab')); print(type(mm.string).__name__)
