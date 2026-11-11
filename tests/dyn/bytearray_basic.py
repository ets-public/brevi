x = bytearray(b'abc')
x.append(100); x += b'!'; x[0] = 65; x[1:2] = b'zz'; del x[-1]
print(x, len(x), x.upper(), x.split(b'z'), list(x), x == b'Azzcd', type(x + b'q'), type(b'q' + x))
for c in x:
    if len(x) < 8: x.append(c)
print(x, bytearray.fromhex('ab'), bytes(x), int(bytearray(b'42')), x.pop(), x.find(b'z'))
import re
s = bytearray(b'abcdefgh')
m = re.search(b'[a-h]+', s)
m2 = re.search(b'[e-h]+', s)
print(m.group(), m2.group(), type(m.group()), m.string is s, m)
s[:] = b'xyz'
print(m.group(), m2.group(), m.span(), m2.groups())
b = bytearray(b'x')
it = re.finditer(b'a', b)
try:
    b.extend(b'x' * 400)
except BufferError as e:
    print('BufferError', e)
print(list(it))
del it
b.extend(b'x' * 4)
print(len(b))
c = bytearray(b'a1b2c3')
it = re.finditer(rb'\d', c)
print(next(it).group())
c[3] = ord('9')
print([m.group() for m in it])
print(re.sub(b'y', bytearray(b'a'), bytearray(b'xyz')), re.split(b':', bytearray(b'a:b::c')), re.findall(b'.', bytearray(b'ab')))
print(re.fullmatch(b'a+', bytearray(b'aa')), re.match(b'(a)(b)?', bytearray(b'ac')).groups())
try:
    re.compile(bytearray(b'a'))
except TypeError as e:
    print('TE', e)
try:
    re.match('a', bytearray(b'a'))
except TypeError as e:
    print('TE', e)
sc = re.compile(b'a').scanner(bytearray(b'aa'))
print(sc.search(), sc.search(), sc.search())
import re
s = bytearray(b'a1b2c3d4')
def f(m):
    s[5] = ord('X')
    return b'<' + m.group() + b'>'
print(re.sub(rb'\d', f, s), s)
s = bytearray(b'a1b2')
def g(m):
    s.append(1)
    return b''
try:
    re.sub(rb'\d', g, s)
except BufferError as e:
    print('BE', e)
s.append(2); print(s)
print(re.subn(b'', lambda m: b'-', bytearray(b'ab')), re.sub(b'a', lambda m: b'z', bytearray(b'aaa'), count=2))
