# CPython's re matches an array.array as the bytes of its buffer; groups are slices of the array itself.
import re, array
a = array.array('b', b'hello world')
m = re.search(b'(w)or', a)
print(m.span(), m.group(), m.group(1), m.groups(), m.string is a, type(m.group()).__name__)
print(re.findall(b'o', a), re.findall(b'(l)(o)', a))
print(re.sub(b'o', b'0', a), re.subn(b'l', b'L', a))
print(re.split(b' ', a))
print([x.span() for x in re.finditer(b'l+', a)])
print(re.fullmatch(b'hello world', a) is not None, re.match(b'world', a))
print(re.compile(b'h(e)').match(a).group(0, 1))
for tc in 'BhHiIlLqQfd':
    x = array.array(tc, [1, 2])
    print(tc, re.match(b'\x01', x) is not None, len(re.findall(b'\x00', x)))
try:
    re.match('h', a)
except TypeError as e:
    print("TypeError", e)
print(re.escape(b'a.b'), re.match(b'', array.array('b')).group())
